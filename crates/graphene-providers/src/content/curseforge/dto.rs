use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurseForgeAuthorDto {
    pub(super) name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurseForgeCategoryDto {
    pub(super) name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurseForgeLogoDto {
    pub(super) thumbnail_url: Option<String>,
    pub(super) url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurseForgeLinksDto {
    pub(super) website_url: Option<String>,
    pub(super) wiki_url: Option<String>,
    pub(super) issues_url: Option<String>,
    pub(super) source_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurseForgeModDto {
    pub(super) id: u32,
    pub(super) name: String,
    pub(super) slug: String,
    pub(super) summary: Option<String>,
    pub(super) links: Option<CurseForgeLinksDto>,
    pub(super) authors: Option<Vec<CurseForgeAuthorDto>>,
    pub(super) logo: Option<CurseForgeLogoDto>,
    pub(super) categories: Option<Vec<CurseForgeCategoryDto>>,
    pub(super) download_count: Option<u64>,
    pub(super) thumbs_up_count: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurseForgeFileHashDto {
    pub(super) value: String,
    pub(super) algo: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurseForgeFileDependencyDto {
    pub(super) mod_id: u32,
    pub(super) relation_type: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurseForgeFileDto {
    pub(super) id: u32,
    pub(super) mod_id: u32,
    pub(super) is_available: Option<bool>,
    pub(super) display_name: String,
    pub(super) file_name: String,
    pub(super) release_type: u32,
    pub(super) file_status: Option<u32>,
    pub(super) hashes: Option<Vec<CurseForgeFileHashDto>>,
    pub(super) file_date: Option<String>,
    pub(super) file_length: u64,
    pub(super) download_count: Option<u64>,
    pub(super) download_url: Option<String>,
    pub(super) game_versions: Option<Vec<String>>,
    pub(super) dependencies: Option<Vec<CurseForgeFileDependencyDto>>,
    pub(super) package_fingerprint: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurseForgePaginationDto {
    pub(super) index: Option<u32>,
    pub(super) page_size: Option<u32>,
    pub(super) total_count: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CurseForgeSearchResponseDto {
    pub(super) data: Vec<CurseForgeModDto>,
    pub(super) pagination: Option<CurseForgePaginationDto>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CurseForgeSingleModResponseDto {
    pub(super) data: CurseForgeModDto,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CurseForgeFilesResponseDto {
    pub(super) data: Vec<CurseForgeFileDto>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CurseForgeSingleFileResponseDto {
    pub(super) data: CurseForgeFileDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurseForgeFingerprintMatchDto {
    pub(super) file: CurseForgeFileDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurseForgeFingerprintsDataDto {
    pub(super) exact_matches: Option<Vec<CurseForgeFingerprintMatchDto>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CurseForgeFingerprintsResponseDto {
    pub(super) data: CurseForgeFingerprintsDataDto,
}
