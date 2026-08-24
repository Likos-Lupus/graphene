use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};

pub const DEFAULT_MODRINTH_API_URL: &str = "https://api.modrinth.com/v2";

/// Configuration for the Modrinth content provider adapter.
#[derive(Clone, PartialEq, Eq)]
pub struct ModrinthProviderConfig {
    pub base_url: String,
    pub allow_http: bool,
    pub user_agent: String,
}

impl std::fmt::Debug for ModrinthProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModrinthProviderConfig")
            .field("base_url", &self.base_url)
            .field("allow_http", &self.allow_http)
            .field("user_agent", &self.user_agent)
            .finish()
    }
}

impl Default for ModrinthProviderConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_MODRINTH_API_URL.to_string(),
            allow_http: false,
            user_agent: "GrapheneLauncher/0.1.0".to_string(),
        }
    }
}

impl ModrinthProviderConfig {
    /// Builds production Modrinth configuration.
    #[must_use]
    pub fn production() -> Self {
        Self::default()
    }

    /// Builds fixture Modrinth configuration with custom endpoint and HTTP allowed.
    pub fn fixture(base_url: impl Into<String>) -> Result<Self> {
        let config = Self {
            base_url: base_url.into(),
            allow_http: true,
            user_agent: "GrapheneTestFixture/0.1.0".to_string(),
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.base_url.is_empty() || self.base_url.len() > 2048 {
            return Err(GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                "Modrinth base URL length is outside valid bounds",
            ));
        }

        let scheme_ok = self.base_url.starts_with("https://")
            || (self.allow_http
                && (self.base_url.starts_with("http://127.0.0.1:")
                    || self.base_url.starts_with("http://localhost:")));

        if !scheme_ok {
            return Err(GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                "Modrinth base URL violates transport security policy",
            ));
        }

        Ok(())
    }
}
