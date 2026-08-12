pub(super) mod jar;

use graphene_core::{
    Artifact, ArtifactId, ArtifactIntegrity, ArtifactKind, ArtifactSource, ErrorCode, ErrorKind,
    GrapheneError, Result,
};

pub const MAX_LOADER_METADATA_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_LOADER_VERSIONS: usize = 4096;
pub const MAX_PROFILE_LIBRARIES: usize = 4096;

pub fn validate_endpoint(value: &str, allow_http: bool) -> Result<()> {
    if value.len() > 4096 || value.contains('\0') {
        return Err(loader_error(
            ErrorCode::ConfigInvalid,
            "loader endpoint is invalid",
        ));
    }

    let https = value.starts_with("https://");
    let http = value.starts_with("http://");

    if !https && !(allow_http && http) {
        return Err(loader_error(
            ErrorCode::ConfigInvalid,
            "loader endpoint must use HTTPS outside explicit fixtures",
        ));
    }

    if value.starts_with("file:") || value.contains('@') {
        return Err(loader_error(
            ErrorCode::ConfigInvalid,
            "loader endpoint is unsafe",
        ));
    }

    if !allow_http && endpoint_host_is_private(value) {
        return Err(loader_error(
            ErrorCode::ConfigInvalid,
            "loader endpoint resolves to a disallowed local/private host form",
        ));
    }

    Ok(())
}

fn endpoint_host_is_private(value: &str) -> bool {
    use std::net::IpAddr;

    let authority = value
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(value)
        .split('/')
        .next()
        .unwrap_or("")
        .trim_end_matches('.');
    let host = if let Some(rest) = authority.strip_prefix('[') {
        rest.split_once(']').map(|(host, _)| host).unwrap_or(rest)
    } else {
        authority.split(':').next().unwrap_or("")
    }
    .to_ascii_lowercase();

    if host.is_empty()
        || host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
    {
        return true;
    }

    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => {
            address.is_private()
                || address.is_loopback()
                || address.is_link_local()
                || address.is_unspecified()
        }
        Ok(IpAddr::V6(address)) => {
            address.is_loopback()
                || address.is_unspecified()
                || address.is_unique_local()
                || address.is_unicast_link_local()
        }
        Err(_) => false,
    }
}

pub fn join_base(base: &str, suffix: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        suffix.trim_start_matches('/')
    )
}

pub fn percent_encode_segment(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            use std::fmt::Write as _;
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

pub fn artifact(
    url: String,
    integrity: ArtifactIntegrity,
    expected_size: Option<u64>,
    allow_http: bool,
) -> Result<Artifact> {
    validate_endpoint(&url, allow_http)?;
    if !integrity.is_verifiable() {
        return Err(loader_error(
            ErrorCode::LoaderArtifactUnverifiable,
            "loader artifact has no trustworthy declared integrity",
        ));
    }

    let source = ArtifactSource::new(url).with_label("loader-provider");
    let mut artifact = Artifact::new(vec![source], integrity.clone());

    artifact.kind = ArtifactKind::Binary;
    artifact.expected_size = expected_size;

    let mut id = [0_u8; 16];
    if let Some(sha256) = integrity.sha256() {
        id.copy_from_slice(&sha256.as_bytes()[..16]);
    } else if let Some(sha1) = integrity.sha1() {
        id.copy_from_slice(&sha1.as_bytes()[..16]);
    }

    artifact.id = ArtifactId::from_bytes(id);
    Ok(artifact)
}

pub fn loader_error(code: ErrorCode, message: &'static str) -> GrapheneError {
    GrapheneError::new(code, ErrorKind::Minecraft, message)
}

#[cfg(test)]
mod endpoint_tests {
    use super::validate_endpoint;

    #[test]
    fn production_endpoints_reject_local_private_hosts() {
        for value in [
            "https://localhost/path",
            "https://127.0.0.1/path",
            "https://10.0.0.1/path",
            "https://172.16.1.1/path",
            "https://192.168.0.1/path",
            "https://169.254.1.1/path",
            "https://[::1]/path",
            "https://[fc00::1]/path",
            "https://[fe80::1]/path",
            "https://0.0.0.0/path",
        ] {
            assert!(validate_endpoint(value, false).is_err(), "{value}");
        }
        assert!(validate_endpoint("https://maven.fabricmc.net/", false).is_ok());
        assert!(validate_endpoint("http://127.0.0.1:1234/fixture", true).is_ok());
    }
}
