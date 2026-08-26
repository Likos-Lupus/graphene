//! Private `.mrpack` manifest DTOs. These types never leave this format module.

use serde::Deserialize;

pub(super) const MAX_DEPENDENCIES: usize = 32;
pub(super) const MAX_DOWNLOAD_URLS_PER_FILE: usize = 16;

#[derive(Debug, Deserialize)]
pub(super) struct ModrinthIndexDto {
    #[serde(rename = "formatVersion")]
    pub format_version: u64,
    pub game: String,
    #[serde(rename = "versionId")]
    pub version_id: Option<String>,
    pub name: Option<String>,
    pub summary: Option<String>,
    pub files: Vec<ModrinthFileDto>,
    pub dependencies: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum ModrinthEnvValue {
    Required,
    Optional,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub(super) struct ModrinthEnvDto {
    pub client: Option<ModrinthEnvValue>,
    pub server: Option<ModrinthEnvValue>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ModrinthFileDto {
    pub path: String,
    pub hashes: ModrinthHashesDto,
    pub env: Option<ModrinthEnvDto>,
    pub downloads: Vec<String>,
    #[serde(rename = "fileSize")]
    pub file_size: u64,
}

#[derive(Debug, Deserialize)]
pub(super) struct ModrinthHashesDto {
    pub sha1: String,
    pub sha512: String,
}
