use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ModrinthSearchHitDto {
    pub(super) project_id: String,
    pub(super) slug: String,
    pub(super) title: String,
    pub(super) description: Option<String>,
    pub(super) icon_url: Option<String>,
    pub(super) author: Option<String>,
    pub(super) categories: Option<Vec<String>>,
    pub(super) downloads: Option<u64>,
    pub(super) follows: Option<u64>,
    pub(super) versions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ModrinthSearchResponseDto {
    pub(super) hits: Vec<ModrinthSearchHitDto>,
    pub(super) offset: Option<u32>,
    pub(super) limit: Option<u32>,
    pub(super) total_hits: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ModrinthProjectDto {
    pub(super) id: String,
    pub(super) slug: String,
    pub(super) title: String,
    pub(super) description: Option<String>,
    pub(super) icon_url: Option<String>,
    pub(super) source_url: Option<String>,
    pub(super) issues_url: Option<String>,
    pub(super) wiki_url: Option<String>,
    pub(super) discord_url: Option<String>,
    pub(super) categories: Option<Vec<String>>,
    pub(super) downloads: Option<u64>,
    pub(super) followers: Option<u64>,
    pub(super) client_side: Option<String>,
    pub(super) server_side: Option<String>,
    pub(super) game_versions: Option<Vec<String>>,
    pub(super) loaders: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ModrinthVersionFileHashesDto {
    pub(super) sha1: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ModrinthVersionFileDto {
    pub(super) hashes: ModrinthVersionFileHashesDto,
    pub(super) url: String,
    pub(super) filename: String,
    pub(super) primary: Option<bool>,
    pub(super) size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ModrinthDependencyDto {
    pub(super) version_id: Option<String>,
    pub(super) project_id: Option<String>,
    pub(super) file_name: Option<String>,
    pub(super) dependency_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ModrinthVersionDto {
    pub(super) id: String,
    pub(super) project_id: String,
    pub(super) name: Option<String>,
    pub(super) version_number: String,
    pub(super) version_type: Option<String>,
    pub(super) game_versions: Vec<String>,
    pub(super) loaders: Vec<String>,
    pub(super) dependencies: Option<Vec<ModrinthDependencyDto>>,
    pub(super) files: Vec<ModrinthVersionFileDto>,
    pub(super) date_published: Option<String>,
    pub(super) downloads: Option<u64>,
    pub(super) status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ModrinthVersionFilesMatchDto {
    #[serde(flatten)]
    pub(super) matches: HashMap<String, ModrinthVersionDto>,
}
