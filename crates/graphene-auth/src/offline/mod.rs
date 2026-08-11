use crate::{
    Account, AccountKind, AccountProfile, AccountState, AuthSession, OfflineAccountSpec,
    error::auth_error,
};
use graphene_core::{AccountId, ErrorCode, Result, SensitiveString};
use uuid::Uuid;

pub fn validate_offline_name(value: &str) -> Result<()> {
    let valid = (1..=16).contains(&value.len())
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    if !valid {
        return Err(auth_error(
            ErrorCode::AuthRequestInvalid,
            "offline display name must be 1-16 ASCII letters, digits, or underscore",
        ));
    }
    Ok(())
}

pub fn create_offline_account(spec: OfflineAccountSpec) -> Result<Account> {
    validate_offline_name(&spec.display_name)?;
    let account = Account {
        id: AccountId::new(),
        kind: AccountKind::Offline,
        profile: AccountProfile {
            display_name: spec.display_name,
            minecraft_uuid: spec.profile_uuid.unwrap_or_else(Uuid::new_v4),
        },
        state: AccountState::Ready,
    };
    account.validate()?;
    Ok(account)
}

pub fn offline_auth_session(account: &Account) -> Result<AuthSession> {
    if account.kind != AccountKind::Offline || account.state != AccountState::Ready {
        return Err(auth_error(
            ErrorCode::AuthAccountStateInvalid,
            "account is not a ready offline identity",
        ));
    }
    Ok(AuthSession {
        kind: AccountKind::Offline,
        profile: account.profile.clone(),
        access_token: SensitiveString::new("0"),
        refresh_credential: None,
        client_id: None,
        xuid: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_names_are_minecraft_compatible() {
        assert!(validate_offline_name("Player_123").is_ok());
        assert!(validate_offline_name("has space").is_err());
        assert!(validate_offline_name("abcdefghijklmnopq").is_err());
    }

    #[test]
    fn supplied_uuid_is_stable_identity() {
        let id = Uuid::new_v4();
        let account =
            create_offline_account(OfflineAccountSpec::new("Player").with_profile_uuid(id))
                .unwrap();
        assert_eq!(account.profile.minecraft_uuid, id);
    }
}
