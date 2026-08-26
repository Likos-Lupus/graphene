use crate::{InstalledComponent, ManagedRelativePath};
use graphene_core::{
    ArtifactIntegrity, ArtifactKind, ArtifactSource, GrapheneError, InstanceId, Sha256Digest,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const LOCKFILE_SCHEMA_VERSION: u32 = 3;
pub const OLDEST_READABLE_LOCKFILE_SCHEMA_VERSION: u32 = 1;
pub const MAX_LOCKED_COMPONENTS: usize = 64;
pub const MAX_LOCKED_ARTIFACTS: usize = 4096;
pub const MAX_LOCKED_OUTPUTS: usize = 256;
pub const MAX_LOCKED_EXTRACTIONS: usize = 256;
pub const MAX_LOCKED_CONTENT: usize = 1024;
pub const MAX_LOCKED_DEPENDENCIES_PER_ENTRY: usize = 64;
pub const MAX_PACK_ORIGIN_STRING_CHARS: usize = 128;

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

/// Durable snapshot of a dependency relationship for an installed content entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedContentDependency {
    pub target: String,
    pub relation: String,
}

/// Durable record of a Graphene-managed content entry (e.g. mod) persisted in desired state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedContentEntry {
    pub entry_id: String,
    pub kind: String,
    pub provider: Option<String>,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub file_id: Option<String>,
    pub artifact_logical_key: String,
    pub destination: ManagedRelativePath,
    pub enabled: bool,
    #[serde(default)]
    pub dependencies: Vec<LockedContentDependency>,
}

/// Bounded historical provenance describing how an instance was originally created from an
/// external pack. This is never a second desired-state authority: current bytes remain the
/// `artifacts`, `generated_outputs`, and `content` lists. Raw source URLs, local host paths,
/// credentials, and full upstream manifests are deliberately unrepresentable here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedPackOrigin {
    /// Normalized Graphene pack format identity (e.g. `MODRINTH`, `CURSEFORCE`).
    pub format: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub source_sha256: Sha256Digest,
    pub external_project_id: Option<String>,
    pub external_version_id: Option<String>,
}

impl LockedPackOrigin {
    /// Creates a bounded origin record after validating every string field.
    pub fn new(
        format: impl Into<String>,
        name: Option<String>,
        version: Option<String>,
        source_sha256: Sha256Digest,
        external_project_id: Option<String>,
        external_version_id: Option<String>,
    ) -> Result<Self, GrapheneError> {
        let format = format.into();
        if format.is_empty() || format.chars().count() > MAX_PACK_ORIGIN_STRING_CHARS {
            return Err(crate::error::instance_error(
                "pack origin format is missing or exceeds its bound",
            ));
        }
        for field in [&name, &version] {
            if let Some(value) = field
                && (value.is_empty()
                    || value.chars().count() > MAX_PACK_ORIGIN_STRING_CHARS
                    || value.chars().any(char::is_control))
            {
                return Err(crate::error::instance_error(
                    "pack origin display field is empty, control-bearing, or exceeds its bound",
                ));
            }
        }
        for field in [&external_project_id, &external_version_id] {
            if let Some(value) = field
                && (value.is_empty()
                    || value.chars().count() > MAX_PACK_ORIGIN_STRING_CHARS
                    || value
                        .chars()
                        .any(|c| c.is_control() || c == '/' || c == '\\'))
            {
                return Err(crate::error::instance_error(
                    "pack origin external id is empty, path-like, or exceeds its bound",
                ));
            }
        }
        Ok(Self {
            format,
            name,
            version,
            source_sha256,
            external_project_id,
            external_version_id,
        })
    }

    /// Re-validates a deserialized origin record against the same bounds as [`Self::new`].
    pub fn validate(&self) -> Result<(), GrapheneError> {
        Self::new(
            self.format.clone(),
            self.name.clone(),
            self.version.clone(),
            self.source_sha256,
            self.external_project_id.clone(),
            self.external_version_id.clone(),
        )
        .map(|_| ())
    }
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
    #[serde(default)]
    pub content: Vec<LockedContentEntry>,
    #[serde(default)]
    pub pack_origin: Option<LockedPackOrigin>,
}
