use crate::{InstalledComponent, ManagedRelativePath};
use graphene_core::{ArtifactIntegrity, ArtifactKind, ArtifactSource, InstanceId, Sha256Digest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const LOCKFILE_SCHEMA_VERSION: u32 = 1;
pub const MAX_LOCKED_COMPONENTS: usize = 64;
pub const MAX_LOCKED_ARTIFACTS: usize = 4096;
pub const MAX_LOCKED_OUTPUTS: usize = 256;
pub const MAX_LOCKED_EXTRACTIONS: usize = 256;

/// Materialization scope and replacement strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LockedMaterializationScope {
    /// Placed under data-root `shared/` (e.g. libraries, assets). Shared across instances.
    SharedImmutable,
    /// Placed inside instance working directory (e.g. client JAR).
    InstanceMutable,
}

/// Durable record of an artifact required by the instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedArtifact {
    pub logical_key: String,
    pub kind: ArtifactKind,
    pub sources: Vec<ArtifactSource>,
    pub destination: ManagedRelativePath,
    pub scope: LockedMaterializationScope,
    pub integrity: ArtifactIntegrity,
    pub expected_size: Option<u64>,
}

/// Durable native archive extraction mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedNativeExtraction {
    pub archive_path: ManagedRelativePath,
    pub destination_dir: ManagedRelativePath,
    pub exclude_patterns: Vec<String>,
}

/// Durable record for a generated loader output with cryptographic provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedGeneratedOutput {
    pub destination: ManagedRelativePath,
    pub sha256: Sha256Digest,
    pub size: u64,
    pub component_uid: String,
    pub component_version: String,
    pub provider: String,
    pub input_sha256: BTreeMap<String, String>,
}

/// Durable, provider-neutral desired state lockfile persisted at `instances/<id>/.graphene/lock.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceLockfile {
    pub schema_version: u32,
    pub instance_id: InstanceId,
    pub minecraft_version: String,
    pub components: Vec<InstalledComponent>,
    pub artifacts: Vec<LockedArtifact>,
    pub native_extractions: Vec<LockedNativeExtraction>,
    pub generated_outputs: Vec<LockedGeneratedOutput>,
}
