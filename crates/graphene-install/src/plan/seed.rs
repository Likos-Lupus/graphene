use graphene_core::{ArtifactId, ErrorCode, Sha256Digest};
use graphene_minecraft::ManagedPath;

pub const MAX_SEED_ARCHIVE_LAYERS: usize = 20_000;
pub const MAX_SEED_ENTRY_NAME_BYTES: usize = 1_024;

/// One ordered extraction from a cached archive into the instance staging tree.
///
/// The archive is identified by an already-acquired artifact id (for pack imports this is the
/// immutable source snapshot); Graphene never re-derives it from a path or URL at execution time.
/// Every layer carries its expected size and SHA-256 so the executor can verify each written file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SeedArchiveLayer {
    pub archive_artifact_id: ArtifactId,
    pub archive_entry: String,
    pub destination: ManagedPath,
    pub expected_size: u64,
    pub expected_sha256: Sha256Digest,
}

impl SeedArchiveLayer {
    /// Validates bounded identity fields before the layer may join an install plan.
    pub fn new(
        archive_artifact_id: ArtifactId,
        archive_entry: impl Into<String>,
        destination: ManagedPath,
        expected_size: u64,
        expected_sha256: Sha256Digest,
    ) -> Result<Self, graphene_core::GrapheneError> {
        let archive_entry = archive_entry.into();
        if archive_entry.is_empty()
            || archive_entry.len() > MAX_SEED_ENTRY_NAME_BYTES
            || archive_entry.chars().any(char::is_control)
            || archive_entry.contains('\0')
        {
            return Err(graphene_core::GrapheneError::new(
                ErrorCode::InstallPlanInvalid,
                graphene_core::ErrorKind::Install,
                "seed archive entry name is empty, control-bearing, or unbounded",
            ));
        }
        Ok(Self {
            archive_artifact_id,
            archive_entry,
            destination,
            expected_size,
            expected_sha256,
        })
    }
}
