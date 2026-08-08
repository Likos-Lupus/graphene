use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub(super) struct ManifestDto {
    pub(super) latest: LatestDto,
    #[serde(default)]
    pub(super) versions: Vec<ManifestVersionDto>,
}

#[derive(Deserialize)]
pub(super) struct LatestDto {
    pub(super) release: String,
    pub(super) snapshot: String,
}

#[derive(Deserialize)]
pub(super) struct ManifestVersionDto {
    pub(super) id: String,
    #[serde(rename = "type")]
    pub(super) version_type: String,
    pub(super) url: String,
    pub(super) sha1: Option<String>,
    pub(super) size: Option<u64>,
    #[serde(rename = "releaseTime")]
    pub(super) release_time: Option<String>,
    #[serde(rename = "complianceLevel")]
    pub(super) compliance_level: Option<u32>,
}

#[derive(Deserialize)]
pub(super) struct VersionDto {
    pub(super) id: String,
    #[serde(rename = "type", default = "default_release_type")]
    pub(super) version_type: String,
    #[serde(rename = "inheritsFrom")]
    pub(super) inherits_from: Option<String>,
    #[serde(rename = "mainClass")]
    pub(super) main_class: Option<String>,
    pub(super) downloads: Option<VersionDownloadsDto>,
    pub(super) arguments: Option<ArgumentsDto>,
    #[serde(rename = "minecraftArguments")]
    pub(super) minecraft_arguments: Option<String>,
    #[serde(default)]
    pub(super) libraries: Vec<LibraryDto>,
    #[serde(rename = "assetIndex")]
    pub(super) asset_index: Option<AssetIndexReferenceDto>,
    pub(super) assets: Option<String>,
    pub(super) logging: Option<LoggingDto>,
    #[serde(rename = "javaVersion")]
    pub(super) java_version: Option<JavaVersionDto>,
    #[serde(rename = "releaseTime")]
    pub(super) release_time: Option<String>,
    #[serde(rename = "complianceLevel")]
    pub(super) compliance_level: Option<u32>,
}

pub(super) fn default_release_type() -> String {
    "release".to_owned()
}

#[derive(Deserialize)]
pub(super) struct VersionDownloadsDto {
    pub(super) client: Option<DownloadDto>,
}

#[derive(Deserialize, Clone)]
pub(super) struct DownloadDto {
    pub(super) sha1: String,
    pub(super) size: u64,
    pub(super) url: String,
    pub(super) path: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct ArgumentsDto {
    #[serde(default)]
    pub(super) game: Vec<ArgumentDto>,
    #[serde(default)]
    pub(super) jvm: Vec<ArgumentDto>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum ArgumentDto {
    Literal(String),
    Conditional {
        #[serde(default)]
        rules: Vec<RuleDto>,
        value: ArgumentValueDto,
    },
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum ArgumentValueDto {
    One(String),
    Many(Vec<String>),
}

impl ArgumentValueDto {
    pub(super) fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value],
            Self::Many(values) => values,
        }
    }
}

#[derive(Deserialize)]
pub(super) struct RuleDto {
    pub(super) action: String,
    pub(super) os: Option<OsRuleDto>,
    #[serde(default)]
    pub(super) features: BTreeMap<String, bool>,
}

#[derive(Deserialize)]
pub(super) struct OsRuleDto {
    pub(super) name: Option<String>,
    pub(super) arch: Option<String>,
    pub(super) version: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct LibraryDto {
    pub(super) name: String,
    #[serde(default)]
    pub(super) rules: Vec<RuleDto>,
    pub(super) downloads: Option<LibraryDownloadsDto>,
    #[serde(default)]
    pub(super) natives: BTreeMap<String, String>,
}

#[derive(Deserialize)]
pub(super) struct LibraryDownloadsDto {
    pub(super) artifact: Option<DownloadDto>,
    #[serde(default)]
    pub(super) classifiers: BTreeMap<String, DownloadDto>,
}

#[derive(Deserialize)]
pub(super) struct AssetIndexReferenceDto {
    pub(super) id: String,
    pub(super) sha1: String,
    pub(super) size: u64,
    pub(super) url: String,
    #[serde(rename = "totalSize")]
    pub(super) total_size: Option<u64>,
}

impl AssetIndexReferenceDto {
    pub(super) fn into_download(self) -> DownloadDto {
        let _ = self.total_size;
        DownloadDto {
            sha1: self.sha1,
            size: self.size,
            url: self.url,
            path: None,
        }
    }
}

#[derive(Deserialize)]
pub(super) struct AssetIndexDto {
    #[serde(default)]
    pub(super) objects: BTreeMap<String, AssetObjectDto>,
    #[serde(rename = "virtual", default)]
    pub(super) virtual_layout: bool,
    #[serde(rename = "map_to_resources", default)]
    pub(super) map_to_resources: bool,
}

#[derive(Deserialize)]
pub(super) struct AssetObjectDto {
    pub(super) hash: String,
    pub(super) size: u64,
}

#[derive(Deserialize)]
pub(super) struct LoggingDto {
    pub(super) client: Option<LoggingClientDto>,
}

#[derive(Deserialize)]
pub(super) struct LoggingClientDto {
    pub(super) argument: String,
    pub(super) file: LoggingFileDto,
}

#[derive(Deserialize)]
pub(super) struct LoggingFileDto {
    pub(super) id: String,
    #[serde(flatten)]
    pub(super) download: DownloadDto,
}

#[derive(Deserialize)]
pub(super) struct JavaVersionDto {
    pub(super) component: Option<String>,
    #[serde(rename = "majorVersion")]
    pub(super) major_version: u32,
}
