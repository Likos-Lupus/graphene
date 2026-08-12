use crate::loader::common::{MAX_LOADER_METADATA_BYTES, validate_endpoint};
use graphene_core::Result;

#[derive(Debug, Clone)]
pub struct ForgeProviderConfig {
    pub promotions_url: String,
    pub maven_base: String,
    pub max_metadata_bytes: usize,
    pub allow_http: bool,
}

impl Default for ForgeProviderConfig {
    fn default() -> Self {
        Self {
            promotions_url:
                "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json"
                    .to_owned(),
            maven_base: "https://maven.minecraftforge.net".to_owned(),
            max_metadata_bytes: MAX_LOADER_METADATA_BYTES,
            allow_http: false,
        }
    }
}

impl ForgeProviderConfig {
    pub fn fixture(promotions_url: String, maven_base: String) -> Result<Self> {
        let config = Self {
            promotions_url,
            maven_base,
            max_metadata_bytes: MAX_LOADER_METADATA_BYTES,
            allow_http: true,
        };
        config.validate()?;
        Ok(config)
    }

    pub(super) fn validate(&self) -> Result<()> {
        validate_endpoint(&self.promotions_url, self.allow_http)?;
        validate_endpoint(&self.maven_base, self.allow_http)
    }
}
