use super::{config::MojangProviderConfig, error::mc_error};
use graphene_core::{ErrorCode, Result};

const MAX_STRING_BYTES: usize = 4096;

pub(super) fn normalized_source_url(value: &str, config: &MojangProviderConfig) -> Result<String> {
    bounded_string(value, "artifact URL")?;
    if let Some(base) = &config.fixture_source_base
        && let Some(suffix) = value.strip_prefix("https://fixture.invalid")
    {
        let rewritten = format!("{}{}", base.trim_end_matches('/'), suffix);
        validate_endpoint(&rewritten, true)?;
        return Ok(rewritten);
    }

    validate_endpoint(value, config.allow_http)?;
    Ok(value.to_owned())
}

pub(super) fn endpoint_origin(value: &str) -> Result<String> {
    validate_endpoint(value, true)?;
    let scheme_end = value.find("://").ok_or_else(|| {
        mc_error(
            ErrorCode::MinecraftMetadataInvalid,
            "provider endpoint is missing a URL scheme",
        )
    })? + 3;
    let authority_end = value[scheme_end..]
        .find(['/', '?', '#'])
        .map(|offset| scheme_end + offset)
        .unwrap_or(value.len());
    Ok(value[..authority_end].to_owned())
}

pub(super) fn validate_endpoint(value: &str, allow_http: bool) -> Result<()> {
    bounded_string(value, "provider endpoint")?;
    let (scheme, rest) = if let Some(rest) = value.strip_prefix("https://") {
        ("https", rest)
    } else if allow_http {
        if let Some(rest) = value.strip_prefix("http://") {
            ("http", rest)
        } else {
            return Err(mc_error(
                ErrorCode::MinecraftMetadataInvalid,
                "provider endpoint uses an unsafe URL scheme",
            ));
        }
    } else {
        return Err(mc_error(
            ErrorCode::MinecraftMetadataInvalid,
            "provider endpoint must use HTTPS",
        ));
    };

    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') || authority.chars().any(char::is_whitespace)
    {
        return Err(mc_error(
            ErrorCode::MinecraftMetadataInvalid,
            "provider endpoint uses an unsafe authority form",
        ));
    }

    if scheme == "http" && !allow_http {
        return Err(mc_error(
            ErrorCode::MinecraftMetadataInvalid,
            "plain HTTP provider endpoint is disabled",
        ));
    }

    Ok(())
}
pub(super) fn bounded_string(value: &str, field: &'static str) -> Result<()> {
    if value.is_empty() || value.len() > MAX_STRING_BYTES || value.contains('\0') {
        return Err(mc_error(
            ErrorCode::MinecraftMetadataInvalid,
            "provider metadata string is empty or exceeds its bound",
        )
        .with_context("field", field));
    }

    Ok(())
}

pub(super) fn bounded_owned(value: String, field: &'static str) -> Result<String> {
    bounded_string(&value, field)?;
    Ok(value)
}
