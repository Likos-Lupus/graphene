use crate::{
    id::ContentVersionRef,
    model::{dependency::ContentDependency, file::ContentFile, release::ReleaseChannel},
};
use graphene_minecraft::LoaderKind;
use serde::{Deserialize, Serialize};

/// Target environment side support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EnvironmentSupport {
    #[default]
    Both,
    ClientOnly,
    ServerOnly,
    Unknown,
}

impl EnvironmentSupport {
    #[must_use]
    pub const fn supports_client(&self) -> bool {
        matches!(self, Self::Both | Self::ClientOnly | Self::Unknown)
    }
}

/// Normalized project version metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentVersion {
    pub version_ref: ContentVersionRef,
    pub version_number: String,
    pub display_name: String,
    pub release_channel: ReleaseChannel,
    pub game_versions: Vec<String>,
    pub loaders: Vec<LoaderKind>,
    pub environment: EnvironmentSupport,
    pub dependencies: Vec<ContentDependency>,
    pub files: Vec<ContentFile>,
    pub date_published: Option<String>,
    pub downloads: Option<u64>,
    pub available: bool,
}

impl ContentVersion {
    /// Returns the primary verifiable file for this version, or None if ambiguous / unavailable.
    #[must_use]
    pub fn primary_file(&self) -> Option<&ContentFile> {
        let verifiable: Vec<&ContentFile> =
            self.files.iter().filter(|f| f.is_verifiable()).collect();
        if verifiable.is_empty() {
            return None;
        }
        // Look for explicitly marked primary file
        if let Some(primary) = verifiable
            .iter()
            .find(|f| f.role == crate::model::file::FileRole::Primary)
        {
            return Some(primary);
        }
        // If exactly one verifiable file exists, use it
        if verifiable.len() == 1 {
            return Some(verifiable[0]);
        }
        None
    }
}
