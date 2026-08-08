use crate::error::launch_error;
use graphene_core::{ErrorCode, Result, SensitiveString};

const MAX_SESSION_FIELD_BYTES: usize = 512;

/// Ephemeral externally supplied session. Secret wrappers are deliberately non-serializable.
#[derive(Clone, PartialEq, Eq)]
pub struct LaunchSession {
    pub username: String,
    pub uuid: String,
    pub access_token: SensitiveString,
    pub user_type: String,
    pub client_id: Option<SensitiveString>,
    pub xuid: Option<SensitiveString>,
}

impl std::fmt::Debug for LaunchSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LaunchSession")
            .field("username", &self.username)
            .field("uuid", &self.uuid)
            .field("access_token", &"<redacted>")
            .field("user_type", &self.user_type)
            .field("client_id", &self.client_id.as_ref().map(|_| "<redacted>"))
            .field("xuid", &self.xuid.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl LaunchSession {
    pub fn validate(&self) -> Result<()> {
        for (field, value) in [
            ("username", self.username.as_str()),
            ("uuid", self.uuid.as_str()),
            ("user_type", self.user_type.as_str()),
        ] {
            if value.trim().is_empty()
                || value.len() > MAX_SESSION_FIELD_BYTES
                || value.contains('\0')
            {
                return Err(launch_error(
                    ErrorCode::LaunchSessionInvalid,
                    "launch session contains an invalid public field",
                )
                .with_context("field", field));
            }
        }

        if self.access_token.is_empty()
            || self
                .client_id
                .as_ref()
                .is_some_and(SensitiveString::is_empty)
            || self.xuid.as_ref().is_some_and(SensitiveString::is_empty)
        {
            return Err(launch_error(
                ErrorCode::LaunchSessionInvalid,
                "launch session contains an empty secret field",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LaunchArgument;

    #[test]
    fn session_and_secret_argument_debug_are_redacted() {
        let secret = "phase1-access-token-never-print";
        let session = LaunchSession {
            username: "Fixture Player".into(),
            uuid: "00000000-0000-0000-0000-000000000001".into(),
            access_token: SensitiveString::new(secret),
            user_type: "msa".into(),
            client_id: Some(SensitiveString::new("client-secret")),
            xuid: Some(SensitiveString::new("xuid-secret")),
        };
        assert!(!format!("{session:?}").contains(secret));
        assert!(
            !format!("{:?}", LaunchArgument::Secret(session.access_token.clone())).contains(secret)
        );
    }
}
