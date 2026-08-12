use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub(super) struct InstallProfileDto {
    pub spec: Option<u32>,
    pub minecraft: Option<String>,
    pub json: Option<String>,
    #[serde(default)]
    pub libraries: Vec<LibraryDto>,
    #[serde(default)]
    pub processors: Vec<ProcessorDto>,
    #[serde(default)]
    pub data: BTreeMap<String, DataFileDto>,
}

#[derive(Debug, Deserialize)]
pub(super) struct DataFileDto {
    pub client: String,
    pub server: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProcessorDto {
    #[serde(default)]
    pub sides: Vec<String>,
    pub jar: String,
    #[serde(default)]
    pub classpath: Vec<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub outputs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct LibraryDto {
    pub name: String,
    pub url: Option<String>,
    pub downloads: Option<LibraryDownloadsDto>,
    #[serde(default)]
    pub rules: Vec<RuleDto>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct LibraryDownloadsDto {
    pub artifact: Option<DownloadDto>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct DownloadDto {
    pub path: Option<String>,
    pub url: Option<String>,
    pub sha1: Option<String>,
    pub sha256: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct VersionDto {
    #[serde(rename = "inheritsFrom")]
    pub inherits_from: Option<String>,
    #[serde(rename = "mainClass")]
    pub main_class: Option<String>,
    #[serde(default)]
    pub libraries: Vec<LibraryDto>,
    #[serde(default)]
    pub arguments: VersionArgumentsDto,
    #[serde(rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    #[serde(rename = "javaVersion")]
    pub java_version: Option<JavaVersionDto>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct VersionArgumentsDto {
    #[serde(default)]
    pub jvm: Vec<ArgumentDto>,
    #[serde(default)]
    pub game: Vec<ArgumentDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(super) enum ArgumentDto {
    Literal(String),
    Conditional {
        #[serde(default)]
        rules: Vec<RuleDto>,
        value: ArgumentValueDto,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(super) enum ArgumentValueDto {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct RuleDto {
    pub action: String,
    pub os: Option<OsRuleDto>,
    #[serde(default)]
    pub features: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct OsRuleDto {
    pub name: Option<String>,
    pub arch: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct JavaVersionDto {
    #[serde(rename = "majorVersion")]
    pub major_version: Option<u32>,
    pub component: Option<String>,
}
