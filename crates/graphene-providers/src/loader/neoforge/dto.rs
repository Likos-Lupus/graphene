use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct VersionsDto {
    #[serde(default)]
    pub versions: Vec<String>,
}
