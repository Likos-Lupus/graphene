use super::{
    config::MojangProviderConfig,
    dto::DownloadDto,
    error::mc_error,
    policy::{bounded_string, normalized_source_url, validate_endpoint},
};
use graphene_core::{
    Artifact, ArtifactId, ArtifactIntegrity, ArtifactKind, ArtifactSource, ErrorCode, Result,
    Sha1Digest,
};
use graphene_minecraft::{ManagedPath, MinecraftVersionId, ResolvedArtifact};
use std::collections::BTreeSet;

pub(super) fn resolved_download(
    download: DownloadDto,
    path: ManagedPath,
    kind: ArtifactKind,
    config: &MojangProviderConfig,
) -> Result<ResolvedArtifact> {
    let sha1 = parse_sha1(&download.sha1)?;
    let url = normalized_source_url(&download.url, config)?;
    let artifact = artifact_for(
        kind,
        url,
        ArtifactIntegrity::none().with_sha1(sha1),
        Some(download.size),
        config.allow_http,
    )?;

    Ok(ResolvedArtifact {
        artifact,
        relative_path: path,
    })
}

pub(super) fn artifact_for(
    kind: ArtifactKind,
    url: String,
    integrity: ArtifactIntegrity,
    size: Option<u64>,
    allow_http: bool,
) -> Result<Artifact> {
    bounded_string(&url, "artifact URL")?;
    validate_endpoint(&url, allow_http)?;

    let mut artifact = Artifact::new(vec![ArtifactSource::new(url)], integrity);
    artifact.id = deterministic_artifact_id(&artifact.integrity, &artifact.sources[0])?;
    artifact.kind = kind;
    artifact.expected_size = size;

    Ok(artifact)
}

fn deterministic_artifact_id(
    integrity: &ArtifactIntegrity,
    source: &ArtifactSource,
) -> Result<ArtifactId> {
    if let Some(sha256) = integrity.sha256() {
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(&sha256.as_bytes()[..16]);
        return Ok(ArtifactId::from_bytes(bytes));
    }

    if let Some(sha1) = integrity.sha1() {
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(&sha1.as_bytes()[..16]);
        return Ok(ArtifactId::from_bytes(bytes));
    }

    // Unverified manifest metadata may be represented but cannot enter verified acquisition.
    // Keep a deterministic identity without exposing the URL: use a tiny stable byte fold.
    let mut bytes = [0_u8; 16];
    for (index, byte) in source.url().as_bytes().iter().copied().enumerate() {
        bytes[index % 16] = bytes[index % 16].wrapping_mul(31).wrapping_add(byte);
    }

    Ok(ArtifactId::from_bytes(bytes))
}

pub(super) fn parse_sha1(value: &str) -> Result<Sha1Digest> {
    parse_sha1_for(
        value,
        ErrorCode::MinecraftMetadataInvalid,
        "metadata contains an invalid SHA-1 digest",
    )
}

pub(super) fn parse_sha1_for(
    value: &str,
    code: ErrorCode,
    message: &'static str,
) -> Result<Sha1Digest> {
    value
        .parse()
        .map_err(|source| mc_error(code, message).with_source(source))
}

pub(super) fn client_path(id: &MinecraftVersionId) -> Result<ManagedPath> {
    ManagedPath::new(format!(".minecraft/versions/{0}/{0}.jar", id.as_str()))
}

pub(super) fn metadata_version_path(id: &MinecraftVersionId) -> Result<ManagedPath> {
    ManagedPath::new(format!("shared/metadata/versions/{}.json", id.as_str()))
}

pub(super) fn deduplicate_resolved_artifacts(values: &mut Vec<ResolvedArtifact>) {
    let mut seen = BTreeSet::new();
    values.retain(|entry| seen.insert(entry.artifact.id));
}
