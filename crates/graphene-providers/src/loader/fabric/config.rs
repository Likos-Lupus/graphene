use crate::loader::common::{MAX_LOADER_METADATA_BYTES, validate_endpoint};
use graphene_core::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FabricProviderConfig {
    pub meta_base: String,
    pub default_maven_base: String,
    pub max_metadata_bytes: usize,
    pub allow_http: bool,
}

impl Default for FabricProviderConfig {
    fn default() -> Self {
        Self {
            meta_base: "https://meta.fabricmc.net".to_owned(),
            default_maven_base: "https://maven.fabricmc.net".to_owned(),
            max_metadata_bytes: MAX_LOADER_METADATA_BYTES,
            allow_http: false,
        }
    }
}

impl FabricProviderConfig {
    pub fn fixture(meta_base: impl Into<String>, maven_base: impl Into<String>) -> Self {
        Self {
            meta_base: meta_base.into(),
            default_maven_base: maven_base.into(),
            max_metadata_bytes: MAX_LOADER_METADATA_BYTES,
            allow_http: true,
        }
    }

    pub fn validate(&self) -> Result<()> {
        validate_endpoint(&self.meta_base, self.allow_http)?;
        validate_endpoint(&self.default_maven_base, self.allow_http)?;

        if !(1024..=16 * 1024 * 1024).contains(&self.max_metadata_bytes) {
            return Err(crate::loader::common::loader_error(
                graphene_core::ErrorCode::ConfigInvalid,
                "Fabric metadata response bound is invalid",
            ));
        }

        Ok(())
    }
}
