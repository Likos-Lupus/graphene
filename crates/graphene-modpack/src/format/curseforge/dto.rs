//! Private CurseForge manifest DTOs. These types never leave this format module.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct CurseForgeManifestDocument {
    #[serde(rename = "manifestType")]
    pub manifest_type: String,
    #[serde(rename = "manifestVersion")]
    pub manifest_version: u64,
    pub name: String,
    pub version: String,
    pub author: String,
    pub minecraft: CurseForgeMinecraftSection,
    pub files: Vec<CurseForgeFileEntry>,
    pub overrides: Option<String>,
    #[serde(rename = "projectID")]
    pub project_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CurseForgeMinecraftSection {
    pub version: String,
    #[serde(rename = "modLoaders")]
    pub mod_loaders: Vec<CurseForgeModLoaderEntry>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CurseForgeModLoaderEntry {
    pub id: String,
    pub primary: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct CurseForgeFileEntry {
    #[serde(rename = "projectID")]
    pub project_id: u64,
    #[serde(rename = "fileID")]
    pub file_id: u64,
    pub required: bool,
}
