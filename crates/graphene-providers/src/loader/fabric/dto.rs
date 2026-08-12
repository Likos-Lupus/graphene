use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct LoaderVersionEnvelope {
    pub loader: LoaderVersionDto,
}

#[derive(Debug, Deserialize)]
pub(super) struct LoaderVersionDto {
    pub version: String,
    pub stable: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProfileDto {
    #[serde(rename = "inheritsFrom")]
    pub inherits_from: String,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    #[serde(default)]
    pub arguments: ArgumentsDto,
    #[serde(default)]
    pub libraries: Vec<LibraryDto>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ArgumentsDto {
    #[serde(default)]
    pub game: Vec<serde_json::Value>,
    #[serde(default)]
    pub jvm: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct LibraryDto {
    pub name: String,
    pub url: Option<String>,
    pub sha1: Option<String>,
    pub sha256: Option<String>,
    pub size: Option<u64>,
}
