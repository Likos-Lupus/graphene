use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result, SensitiveString};

pub const DEFAULT_CURSEFORGE_API_URL: &str = "https://api.curseforge.com/v1";
pub const MINECRAFT_GAME_ID: u32 = 432;

/// Configuration for the CurseForge content provider adapter.
#[derive(Clone, PartialEq, Eq)]
pub struct CurseForgeProviderConfig {
    api_key: Option<SensitiveString>,
    base_url: String,
    allow_http: bool,
    game_id: u32,
}

impl std::fmt::Debug for CurseForgeProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CurseForgeProviderConfig")
            .field(
                "api_key",
                &if self.api_key.is_some() {
                    "<configured>"
                } else {
                    "<none>"
                },
            )
            .field("base_url", &self.base_url)
            .field("allow_http", &self.allow_http)
            .field("game_id", &self.game_id)
            .finish()
    }
}

impl Default for CurseForgeProviderConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: DEFAULT_CURSEFORGE_API_URL.to_string(),
            allow_http: false,
            game_id: MINECRAFT_GAME_ID,
        }
    }
}

impl CurseForgeProviderConfig {
    /// Builds production CurseForge configuration with an optional distributor-supplied API key.
    pub fn production(api_key: Option<SensitiveString>) -> Result<Self> {
        let config = Self {
            api_key,
            base_url: DEFAULT_CURSEFORGE_API_URL.to_string(),
            allow_http: false,
            game_id: MINECRAFT_GAME_ID,
        };
        config.validate()?;
        Ok(config)
    }

    /// Builds fixture CurseForge configuration with custom endpoint and HTTP allowed.
    pub fn fixture(api_key: Option<SensitiveString>, base_url: impl Into<String>) -> Result<Self> {
        let config = Self {
            api_key,
            base_url: base_url.into(),
            allow_http: true,
            game_id: MINECRAFT_GAME_ID,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.base_url.is_empty() || self.base_url.len() > 2048 {
            return Err(GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                "CurseForge base URL length is outside valid bounds",
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
                "CurseForge base URL violates transport security policy",
            ));
        }

        Ok(())
    }

    #[must_use]
    pub fn api_key(&self) -> Option<&SensitiveString> {
        self.api_key.as_ref()
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    #[must_use]
    pub const fn allow_http(&self) -> bool {
        self.allow_http
    }

    #[must_use]
    pub const fn game_id(&self) -> u32 {
        self.game_id
    }
}
