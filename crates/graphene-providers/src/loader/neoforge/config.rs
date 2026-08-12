use crate::loader::common::{MAX_LOADER_METADATA_BYTES, validate_endpoint};
use graphene_core::Result;

#[derive(Debug, Clone)]
pub struct NeoForgeProviderConfig {
    pub versions_url: String,
    pub maven_base: String,
    pub max_metadata_bytes: usize,
    pub allow_http: bool,
}

impl Default for NeoForgeProviderConfig {
    fn default() -> Self {
        Self {
            versions_url:
                "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge"
                    .to_owned(),
            maven_base: "https://maven.neoforged.net/releases".to_owned(),
            max_metadata_bytes: MAX_LOADER_METADATA_BYTES,
            allow_http: false,
        }
    }
}

impl NeoForgeProviderConfig {
    pub fn fixture(versions_url: String, maven_base: String) -> Result<Self> {
        let config = Self {
            versions_url,
            maven_base,
            max_metadata_bytes: MAX_LOADER_METADATA_BYTES,
            allow_http: true,
        };
        config.validate()?;
        Ok(config)
    }

    pub(super) fn validate(&self) -> Result<()> {
        validate_endpoint(&self.versions_url, self.allow_http)?;
        validate_endpoint(&self.maven_base, self.allow_http)
    }
}
