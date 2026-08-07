use crate::RetryPolicy;
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use reqwest::header::HeaderValue;
use std::{fmt, time::Duration};

/// Bounded redirect policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RedirectPolicy {
    /// Maximum followed redirects for one request.
    pub max_redirects: usize,
}

impl Default for RedirectPolicy {
    fn default() -> Self {
        Self { max_redirects: 10 }
    }
}

/// Proxy behavior. Explicit proxy URLs are treated as sensitive and are redacted from `Debug`.
#[derive(Clone, PartialEq, Eq, Default)]
pub enum ProxyPolicy {
    /// Use transport/system default proxy discovery.
    #[default]
    System,
    /// Disable proxy use.
    None,
    /// Use one explicit HTTP/HTTPS proxy URL.
    Explicit(String),
}

impl fmt::Debug for ProxyPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::System => f.write_str("System"),
            Self::None => f.write_str("None"),
            Self::Explicit(_) => f.write_str("Explicit(<redacted>)"),
        }
    }
}

impl ProxyPolicy {
    pub(crate) fn explicit_url(&self) -> Option<&str> {
        match self {
            Self::Explicit(url) => Some(url),
            Self::System | Self::None => None,
        }
    }
}

/// Graphene-owned network policy. Concrete HTTP client configuration is intentionally hidden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkConfig {
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub retry_policy: RetryPolicy,
    pub proxy: ProxyPolicy,
    pub redirect_policy: RedirectPolicy,
    pub user_agent: String,
    pub max_concurrent_downloads: usize,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(60),
            retry_policy: RetryPolicy::default(),
            proxy: ProxyPolicy::System,
            redirect_policy: RedirectPolicy::default(),
            user_agent: format!("graphene/{}", env!("CARGO_PKG_VERSION")),
            max_concurrent_downloads: 8,
        }
    }
}

impl NetworkConfig {
    /// Validates all resource bounds before engine construction completes.
    pub fn validate(&self) -> Result<()> {
        if self.connect_timeout.is_zero() || self.connect_timeout > Duration::from_secs(120) {
            return Err(config_error(
                "connect_timeout must be between 1 nanosecond and 120 seconds",
            ));
        }

        if self.request_timeout.is_zero() || self.request_timeout > Duration::from_secs(600) {
            return Err(config_error(
                "request_timeout must be between 1 nanosecond and 600 seconds",
            ));
        }

        if !(1..=20).contains(&self.redirect_policy.max_redirects) {
            return Err(config_error("max_redirects must be between 1 and 20"));
        }

        if !(1..=64).contains(&self.max_concurrent_downloads) {
            return Err(config_error(
                "max_concurrent_downloads must be between 1 and 64",
            ));
        }

        if self.user_agent.trim().is_empty() || self.user_agent.len() > 256 {
            return Err(config_error("user_agent must contain 1 to 256 bytes"));
        }

        if self.user_agent.parse::<HeaderValue>().is_err() {
            return Err(config_error(
                "user_agent contains invalid HTTP header bytes",
            ));
        }

        self.retry_policy.validate()?;
        if let Some(proxy) = self.proxy.explicit_url() {
            validate_explicit_proxy(proxy)?;
        }

        Ok(())
    }
}

fn validate_explicit_proxy(proxy: &str) -> Result<()> {
    if proxy.trim().is_empty() {
        return Err(config_error("explicit proxy URL must not be empty"));
    }

    reqwest::Proxy::all(proxy).map_err(|_source| {
        GrapheneError::new(
            ErrorCode::NetworkProxyInvalid,
            ErrorKind::Configuration,
            "explicit proxy configuration is invalid",
        )
    })?;
    Ok(())
}

fn config_error(message: &'static str) -> GrapheneError {
    GrapheneError::new(ErrorCode::ConfigInvalid, ErrorKind::Configuration, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_zero_limits_are_rejected() {
        let mut config = NetworkConfig::default();
        config.max_concurrent_downloads = 0;
        assert_eq!(
            config.validate().expect_err("invalid").code,
            ErrorCode::ConfigInvalid
        );

        let mut config = NetworkConfig::default();
        config.redirect_policy.max_redirects = 0;
        assert_eq!(
            config.validate().expect_err("invalid").code,
            ErrorCode::ConfigInvalid
        );
    }

    #[test]
    fn explicit_proxy_debug_redacts_credentials() {
        let proxy = ProxyPolicy::Explicit("http://user:password@example.invalid".into());
        let debug = format!("{proxy:?}");

        assert!(!debug.contains("password"));
        assert!(!debug.contains("user"));
    }

    #[test]
    fn malformed_proxy_and_user_agent_are_rejected_during_validation() {
        let mut proxy = NetworkConfig::default();
        proxy.proxy = ProxyPolicy::Explicit("not a proxy url".into());
        assert_eq!(
            proxy.validate().expect_err("invalid proxy").code,
            ErrorCode::NetworkProxyInvalid
        );

        let mut user_agent = NetworkConfig::default();
        user_agent.user_agent = "graphene\ninvalid".into();
        assert_eq!(
            user_agent.validate().expect_err("invalid user agent").code,
            ErrorCode::ConfigInvalid
        );
    }
}
