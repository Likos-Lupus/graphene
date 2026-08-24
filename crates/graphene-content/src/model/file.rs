use crate::id::ContentFileRef;
use graphene_core::{ArtifactIntegrity, ArtifactSource};
use serde::{Deserialize, Serialize};

/// Role of the file within its parent version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FileRole {
    #[default]
    Primary,
    Alternative,
    AdditionalResource,
}

/// Normalized downloadable file metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentFile {
    pub file_ref: ContentFileRef,
    pub filename: String,
    pub role: FileRole,
    pub size: u64,
    pub integrity: ArtifactIntegrity,
    pub sources: Vec<ArtifactSource>,
    pub available: bool,
    pub murmur2_fingerprint: Option<u32>,
    pub md5: Option<String>,
}

impl ContentFile {
    /// Returns whether this file satisfies Graphene's download integrity and availability policy.
    #[must_use]
    pub fn is_verifiable(&self) -> bool {
        self.available
            && !self.sources.is_empty()
            && self.integrity.is_verifiable()
            && (self.integrity.sha1().is_some() || self.integrity.sha256().is_some())
    }
}
