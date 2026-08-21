use crate::{Account, AccountKind, AccountProfile, RefreshCredential};
use graphene_core::{OperationController, Result, SensitiveString};
use std::{fmt, future::Future, pin::Pin, time::Instant};

#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuthInteraction {
    DeviceAuthorization {
        verification_uri: String,
        user_code: SensitiveString,
        message: Option<SensitiveString>,
        expires_in_seconds: u64,
        poll_interval_seconds: u64,
    },
}

impl fmt::Debug for AuthInteraction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeviceAuthorization {
                verification_uri,
                expires_in_seconds,
                poll_interval_seconds,
                ..
            } => f
                .debug_struct("DeviceAuthorization")
                .field("verification_uri", verification_uri)
                .field("user_code", &"<redacted>")
                .field("message", &"<redacted>")
                .field("expires_in_seconds", expires_in_seconds)
                .field("poll_interval_seconds", poll_interval_seconds)
                .finish(),
        }
    }
}

#[derive(Clone)]
pub struct AuthChallenge {
    pub interaction: AuthInteraction,
    pub provider_state: SensitiveString,
    /// Absolute in-process expiry captured when the provider created the challenge.
    pub expires_at: Instant,
}

impl fmt::Debug for AuthChallenge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthChallenge")
            .field("interaction", &self.interaction)
            .field("provider_state", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthSession {
    pub kind: AccountKind,
    pub profile: AccountProfile,
    pub access_token: SensitiveString,
    pub refresh_credential: Option<RefreshCredential>,
    pub client_id: Option<SensitiveString>,
    pub xuid: Option<SensitiveString>,
}

impl fmt::Debug for AuthSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthSession")
            .field("kind", &self.kind)
            .field("profile", &self.profile)
            .field("access_token", &"<redacted>")
            .field(
                "refresh_credential",
                &self.refresh_credential.as_ref().map(|_| "<redacted>"),
            )
            .field("client_id", &self.client_id.as_ref().map(|_| "<redacted>"))
            .field("xuid", &self.xuid.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthProviderCapabilities {
    pub device_authorization: bool,
    pub refresh: bool,
}

pub type AuthFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

pub trait AuthProvider: Send + Sync {
    fn capabilities(&self) -> AuthProviderCapabilities;

    fn begin<'a>(&'a self, operation: &'a OperationController) -> AuthFuture<'a, AuthChallenge>;

    fn complete<'a>(
        &'a self,
        challenge: AuthChallenge,
        operation: &'a OperationController,
    ) -> AuthFuture<'a, AuthSession>;

    fn refresh<'a>(
        &'a self,
        account: &'a Account,
        credential: &'a RefreshCredential,
        operation: &'a OperationController,
    ) -> AuthFuture<'a, AuthSession>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    const DEVICE: &str = "DEVICE_CODE_DO_NOT_PRINT";
    const ACCESS: &str = "ACCESS_TOKEN_DO_NOT_PRINT";

    #[test]
    fn interaction_debug_redacts_user_code_and_message() {
        let interaction = AuthInteraction::DeviceAuthorization {
            verification_uri: "https://example.invalid/device".into(),
            user_code: SensitiveString::new(DEVICE),
            message: Some(SensitiveString::new(format!("enter {DEVICE}"))),
            expires_in_seconds: 900,
            poll_interval_seconds: 5,
        };
        let text = format!("{interaction:?}");
        assert!(!text.contains(DEVICE));
    }

    #[test]
    fn session_debug_redacts_access_token() {
        let session = AuthSession {
            kind: AccountKind::Microsoft,
            profile: AccountProfile {
                display_name: "Player".into(),
                minecraft_uuid: Uuid::new_v4(),
            },
            access_token: SensitiveString::new(ACCESS),
            refresh_credential: None,
            client_id: None,
            xuid: None,
        };
        assert!(!format!("{session:?}").contains(ACCESS));
    }
}
