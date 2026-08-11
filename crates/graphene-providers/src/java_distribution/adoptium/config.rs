use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoptiumProviderConfig {
    api_base: String,
    allow_http: bool,
}

impl Default for AdoptiumProviderConfig {
    fn default() -> Self {
        Self {
            api_base: "https://api.adoptium.net/v3".into(),
            allow_http: false,
        }
    }
}

impl AdoptiumProviderConfig {
    pub fn fixture(api_base: impl Into<String>) -> Result<Self> {
        let config = Self {
            api_base: api_base.into().trim_end_matches('/').to_owned(),
            allow_http: true,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        let valid = self.api_base.starts_with("https://")
            || (self.allow_http
                && (self.api_base.starts_with("http://127.0.0.1:")
                    || self.api_base.starts_with("http://localhost:")));
        if !valid || self.api_base.len() > 1024 || self.api_base.contains('@') {
            return Err(GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                "Adoptium provider base URL violates transport policy",
            ));
        }
        Ok(())
    }

    pub(super) fn release_url(
        &self,
        major: u32,
        architecture: &str,
        image: &str,
        os: &str,
    ) -> String {
        format!(
            "{}/assets/latest/{major}/hotspot?architecture={architecture}&image_type={image}&os={os}&vendor=eclipse",
            self.api_base
        )
    }

    pub(super) const fn allow_http(&self) -> bool {
        self.allow_http
    }
}
