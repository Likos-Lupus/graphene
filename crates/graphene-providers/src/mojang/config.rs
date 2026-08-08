use super::policy::{endpoint_origin, validate_endpoint};
use graphene_core::Result;

const DEFAULT_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const DEFAULT_ASSET_OBJECT_BASE: &str = "https://resources.download.minecraft.net";

/// Narrow provider configuration. Production defaults are official HTTPS endpoints; plain HTTP is
/// available only through explicit fixture construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MojangProviderConfig {
    pub(super) manifest_url: String,
    pub(super) asset_object_base: String,
    pub(super) allow_http: bool,
    pub(super) fixture_source_base: Option<String>,
}

impl Default for MojangProviderConfig {
    fn default() -> Self {
        Self {
            manifest_url: DEFAULT_MANIFEST_URL.to_owned(),
            asset_object_base: DEFAULT_ASSET_OBJECT_BASE.to_owned(),
            allow_http: false,
            fixture_source_base: None,
        }
    }
}

impl MojangProviderConfig {
    /// Creates deterministic test configuration. This is deliberately not a general provider API.
    pub fn fixture(
        manifest_url: impl Into<String>,
        asset_object_base: impl Into<String>,
    ) -> Result<Self> {
        let manifest_url = manifest_url.into();
        let asset_object_base = asset_object_base.into();
        let config = Self {
            fixture_source_base: Some(endpoint_origin(&manifest_url)?),
            manifest_url,
            asset_object_base,
            allow_http: true,
        };

        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        validate_endpoint(&self.manifest_url, self.allow_http)?;
        validate_endpoint(&self.asset_object_base, self.allow_http)?;

        if let Some(base) = &self.fixture_source_base {
            validate_endpoint(base, true)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_rejects_credential_bearing_endpoint() {
        assert!(
            MojangProviderConfig::fixture(
                "http://user:secret@127.0.0.1/manifest.json",
                "http://127.0.0.1/assets",
            )
            .is_err()
        );
    }
}
