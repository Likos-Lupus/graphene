use crate::error::mc_error;
use graphene_core::{Artifact, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

const MAX_ID_LEN: usize = 128;

/// Validated explicit Minecraft version identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MinecraftVersionId(String);

impl MinecraftVersionId {
    /// Validates a version identifier before it can influence provider selection or paths.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_ID_LEN || value.contains('\0') {
            return Err(mc_error(
                ErrorCode::MinecraftMetadataInvalid,
                "Minecraft version ID is empty or exceeds its bound",
            ));
        }

        if value == "." || value == ".." || value.contains('/') || value.contains('\\') {
            return Err(mc_error(
                ErrorCode::MinecraftMetadataInvalid,
                "Minecraft version ID contains unsafe path syntax",
            ));
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MinecraftVersionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Official-compatible Minecraft version type with forward-compatible preservation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MinecraftVersionType {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
    Other(String),
}

impl MinecraftVersionType {
    #[must_use]
    pub fn from_provider(value: &str) -> Self {
        match value {
            "release" => Self::Release,
            "snapshot" => Self::Snapshot,
            "old_beta" => Self::OldBeta,
            "old_alpha" => Self::OldAlpha,
            other => Self::Other(other.to_owned()),
        }
    }
}

/// Provider-neutral reference to one version metadata document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionSummary {
    pub id: MinecraftVersionId,
    pub version_type: MinecraftVersionType,
    pub metadata: Artifact,
    pub release_time: Option<String>,
    pub compliance_level: Option<u32>,
}

/// Latest IDs are informational only. Phase 1 installation always selects an explicit ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatestVersions {
    pub release: MinecraftVersionId,
    pub snapshot: MinecraftVersionId,
}

/// Normalized official-compatible version manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionSummary>,
}

impl VersionManifest {
    /// Selects exactly the requested version and never silently substitutes "latest".
    pub fn select(&self, id: &MinecraftVersionId) -> Result<&VersionSummary> {
        self.versions
            .iter()
            .find(|entry| &entry.id == id)
            .ok_or_else(|| {
                mc_error(
                    ErrorCode::MinecraftVersionNotFound,
                    "requested Minecraft version was not found in the manifest",
                )
                .with_context("version_id", id.to_string())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_selection_is_explicit() {
        let requested = MinecraftVersionId::new("missing").expect("version");
        let manifest = VersionManifest {
            latest: LatestVersions {
                release: MinecraftVersionId::new("1.0").expect("version"),
                snapshot: MinecraftVersionId::new("1.1-snapshot").expect("version"),
            },
            versions: Vec::new(),
        };

        assert_eq!(
            manifest.select(&requested).expect_err("missing").code,
            ErrorCode::MinecraftVersionNotFound
        );
    }
}
