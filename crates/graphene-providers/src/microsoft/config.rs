use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};

#[derive(Clone, PartialEq, Eq)]
pub struct MicrosoftServiceEndpoints {
    pub xbox_user_auth: String,
    pub xsts_authorize: String,
    pub minecraft_login: String,
    pub minecraft_entitlements: String,
    pub minecraft_profile: String,
    pub xbox_relying_party: String,
    pub xsts_relying_party: String,
}

impl std::fmt::Debug for MicrosoftServiceEndpoints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicrosoftServiceEndpoints")
            .field("xbox_user_auth", &"<configured-url>")
            .field("xsts_authorize", &"<configured-url>")
            .field("minecraft_login", &"<configured-url>")
            .field("minecraft_entitlements", &"<configured-url>")
            .field("minecraft_profile", &"<configured-url>")
            .field("xbox_relying_party", &"<configured>")
            .field("xsts_relying_party", &"<configured>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct MicrosoftAuthConfig {
    client_id: String,
    scopes: Vec<String>,
    device_code_endpoint: String,
    token_endpoint: String,
    services: MicrosoftServiceEndpoints,
    allow_http: bool,
}

impl std::fmt::Debug for MicrosoftAuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicrosoftAuthConfig")
            .field("client_id", &"<configured>")
            .field("scopes", &self.scopes)
            .field("device_code_endpoint", &"<configured-url>")
            .field("token_endpoint", &"<configured-url>")
            .field("services", &self.services)
            .field("allow_http", &self.allow_http)
            .finish()
    }
}

impl MicrosoftAuthConfig {
    /// Builds production identity endpoints from an explicit distributor-owned application identity.
    /// Xbox/Minecraft service endpoints remain explicit adapter configuration because Graphene does
    /// not borrow another launcher's registration or undocumented endpoint policy.
    pub fn production(
        client_id: impl Into<String>,
        tenant: impl Into<String>,
        scopes: Vec<String>,
        services: MicrosoftServiceEndpoints,
    ) -> Result<Self> {
        let tenant = tenant.into();
        if tenant.is_empty()
            || tenant.len() > 128
            || !tenant
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
        {
            return Err(config_error("Microsoft tenant configuration is invalid"));
        }

        let base = format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0");
        let config = Self {
            client_id: client_id.into(),
            scopes,
            device_code_endpoint: format!("{base}/devicecode"),
            token_endpoint: format!("{base}/token"),
            services,
            allow_http: false,
        };

        config.validate()?;
        Ok(config)
    }

    /// Explicit local-fixture constructor. This is the only path that permits HTTP.
    pub fn fixture(
        client_id: impl Into<String>,
        scopes: Vec<String>,
        device_code_endpoint: impl Into<String>,
        token_endpoint: impl Into<String>,
        services: MicrosoftServiceEndpoints,
    ) -> Result<Self> {
        let config = Self {
            client_id: client_id.into(),
            scopes,
            device_code_endpoint: device_code_endpoint.into(),
            token_endpoint: token_endpoint.into(),
            services,
            allow_http: true,
        };

        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.client_id.is_empty()
            || self.client_id.len() > 256
            || self.scopes.is_empty()
            || self.scopes.len() > 32
            || self
                .scopes
                .iter()
                .any(|scope| scope.is_empty() || scope.len() > 256)
            || !self.scopes.iter().any(|scope| scope == "offline_access")
        {
            return Err(config_error(
                "Microsoft application configuration must include a bounded offline_access scope",
            ));
        }

        for endpoint in self.endpoint_values() {
            validate_endpoint(endpoint, self.allow_http)?;
        }

        if !valid_relying_party(&self.services.xbox_relying_party)
            || !valid_relying_party(&self.services.xsts_relying_party)
        {
            return Err(config_error(
                "Microsoft service relying-party configuration is invalid",
            ));
        }

        Ok(())
    }

    pub(crate) fn client_id(&self) -> &str {
        &self.client_id
    }

    pub(crate) fn scope_string(&self) -> String {
        self.scopes.join(" ")
    }

    pub(crate) fn device_code_endpoint(&self) -> &str {
        &self.device_code_endpoint
    }

    pub(crate) fn token_endpoint(&self) -> &str {
        &self.token_endpoint
    }

    pub(crate) fn services(&self) -> &MicrosoftServiceEndpoints {
        &self.services
    }

    pub(crate) const fn allow_http(&self) -> bool {
        self.allow_http
    }

    fn endpoint_values(&self) -> [&str; 7] {
        [
            &self.device_code_endpoint,
            &self.token_endpoint,
            &self.services.xbox_user_auth,
            &self.services.xsts_authorize,
            &self.services.minecraft_login,
            &self.services.minecraft_entitlements,
            &self.services.minecraft_profile,
        ]
    }
}

fn validate_endpoint(value: &str, allow_http: bool) -> Result<()> {
    let scheme_ok = value.starts_with("https://")
        || (allow_http && value.starts_with("http://127.0.0.1:"))
        || (allow_http && value.starts_with("http://localhost:"));
    if !scheme_ok || value.len() > 2048 || value.contains('@') {
        return Err(config_error(
            "Microsoft provider endpoint violates transport policy",
        ));
    }

    Ok(())
}

fn valid_relying_party(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 2048 && !value.chars().any(char::is_control)
}

fn config_error(message: &'static str) -> GrapheneError {
    GrapheneError::new(ErrorCode::ConfigInvalid, ErrorKind::Configuration, message)
}
