use crate::error::auth_error;
use graphene_core::{AccountId, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_ACCOUNT_DISPLAY_NAME_BYTES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AccountKind {
    Microsoft,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AccountState {
    Ready,
    RequiresReauthentication,
    SecretStoreUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountProfile {
    pub display_name: String,
    pub minecraft_uuid: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub kind: AccountKind,
    pub profile: AccountProfile,
    pub state: AccountState,
}

impl Account {
    pub fn validate(&self) -> Result<()> {
        if self.profile.display_name.trim().is_empty()
            || self.profile.display_name.len() > MAX_ACCOUNT_DISPLAY_NAME_BYTES
            || self.profile.display_name.chars().any(char::is_control)
            || self.profile.minecraft_uuid.is_nil()
        {
            return Err(auth_error(
                ErrorCode::AuthAccountStateInvalid,
                "account profile is outside Graphene bounds",
            ));
        }

        if self.kind == AccountKind::Offline && self.state != AccountState::Ready {
            return Err(auth_error(
                ErrorCode::AuthAccountStateInvalid,
                "offline accounts must remain ready local identities",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineAccountSpec {
    pub display_name: String,
    pub profile_uuid: Option<Uuid>,
}

impl OfflineAccountSpec {
    #[must_use]
    pub fn new(display_name: impl Into<String>) -> Self {
        Self {
            display_name: display_name.into(),
            profile_uuid: None,
        }
    }

    #[must_use]
    pub const fn with_profile_uuid(mut self, profile_uuid: Uuid) -> Self {
        self.profile_uuid = Some(profile_uuid);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_serialization_is_provider_neutral() {
        assert_eq!(
            serde_json::to_string(&AccountKind::Microsoft).unwrap(),
            "\"microsoft\""
        );
        assert_eq!(
            serde_json::to_string(&AccountKind::Offline).unwrap(),
            "\"offline\""
        );
    }

    #[test]
    fn offline_state_validation_rejects_reauth_state() {
        let account = Account {
            id: AccountId::new(),
            kind: AccountKind::Offline,
            profile: AccountProfile {
                display_name: "Player".into(),
                minecraft_uuid: Uuid::new_v4(),
            },
            state: AccountState::RequiresReauthentication,
        };
        assert_eq!(
            account.validate().unwrap_err().code,
            ErrorCode::AuthAccountStateInvalid
        );
    }

    #[test]
    fn account_profile_validation_rejects_nil_uuid_and_control_text() {
        let nil_uuid = Account {
            id: AccountId::new(),
            kind: AccountKind::Microsoft,
            profile: AccountProfile {
                display_name: "Player".into(),
                minecraft_uuid: Uuid::nil(),
            },
            state: AccountState::Ready,
        };
        assert_eq!(
            nil_uuid.validate().expect_err("nil UUID must fail").code,
            ErrorCode::AuthAccountStateInvalid
        );

        let control_name = Account {
            id: AccountId::new(),
            kind: AccountKind::Microsoft,
            profile: AccountProfile {
                display_name: "Player\nInjected".into(),
                minecraft_uuid: Uuid::new_v4(),
            },
            state: AccountState::Ready,
        };
        assert_eq!(
            control_name
                .validate()
                .expect_err("control text must fail")
                .code,
            ErrorCode::AuthAccountStateInvalid
        );
    }
}
