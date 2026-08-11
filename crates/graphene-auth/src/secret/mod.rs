use crate::error::auth_error;
use graphene_core::{AccountId, ErrorCode, Result, SensitiveString};
use std::{collections::HashMap, fmt, sync::Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SecretRecordKind {
    MicrosoftRefreshCredential,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SecretRecordIdentity {
    pub account_id: AccountId,
    pub kind: SecretRecordKind,
}

impl SecretRecordIdentity {
    #[must_use]
    pub const fn microsoft_refresh(account_id: AccountId) -> Self {
        Self {
            account_id,
            kind: SecretRecordKind::MicrosoftRefreshCredential,
        }
    }

    #[must_use]
    pub fn key(&self) -> String {
        let kind = match self.kind {
            SecretRecordKind::MicrosoftRefreshCredential => "microsoft-refresh-v1",
        };
        format!("graphene:{kind}:{}", self.account_id)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RefreshCredential(SensitiveString);

impl RefreshCredential {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(SensitiveString::new(value))
    }
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for RefreshCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RefreshCredential(<redacted>)")
    }
}
impl fmt::Display for RefreshCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

pub trait SecretStore: Send + Sync {
    fn get(&self, identity: &SecretRecordIdentity) -> Result<Option<RefreshCredential>>;
    fn put(&self, identity: &SecretRecordIdentity, value: &RefreshCredential) -> Result<()>;
    fn delete(&self, identity: &SecretRecordIdentity) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct InMemorySecretStore {
    records: Mutex<HashMap<String, RefreshCredential>>,
}

impl InMemorySecretStore {
    #[must_use]
    pub fn new_for_tests() -> Self {
        Self::default()
    }
}

impl SecretStore for InMemorySecretStore {
    fn get(&self, identity: &SecretRecordIdentity) -> Result<Option<RefreshCredential>> {
        Ok(self
            .records
            .lock()
            .expect("secret fixture store poisoned")
            .get(&identity.key())
            .cloned())
    }
    fn put(&self, identity: &SecretRecordIdentity, value: &RefreshCredential) -> Result<()> {
        self.records
            .lock()
            .expect("secret fixture store poisoned")
            .insert(identity.key(), value.clone());
        Ok(())
    }
    fn delete(&self, identity: &SecretRecordIdentity) -> Result<()> {
        self.records
            .lock()
            .expect("secret fixture store poisoned")
            .remove(&identity.key());
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct UnavailableSecretStore;

impl SecretStore for UnavailableSecretStore {
    fn get(&self, _: &SecretRecordIdentity) -> Result<Option<RefreshCredential>> {
        Err(unavailable())
    }
    fn put(&self, _: &SecretRecordIdentity, _: &RefreshCredential) -> Result<()> {
        Err(unavailable())
    }
    fn delete(&self, _: &SecretRecordIdentity) -> Result<()> {
        Err(unavailable())
    }
}

fn unavailable() -> graphene_core::GrapheneError {
    auth_error(
        ErrorCode::AuthSecretStoreUnavailable,
        "no secure credential backend is configured",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "PHASE2_REFRESH_TOKEN_DO_NOT_PRINT";

    #[test]
    fn record_identity_uses_graphene_namespace_and_account_id() {
        let account_id = AccountId::new();
        let key = SecretRecordIdentity::microsoft_refresh(account_id).key();
        assert!(key.starts_with("graphene:microsoft-refresh-v1:"));
        assert!(key.ends_with(&account_id.to_string()));
    }

    #[test]
    fn in_memory_store_is_explicit_and_secret_is_redacted() {
        let store = InMemorySecretStore::new_for_tests();
        let identity = SecretRecordIdentity::microsoft_refresh(AccountId::new());
        let credential = RefreshCredential::new(TOKEN);
        store.put(&identity, &credential).unwrap();
        assert_eq!(
            store.get(&identity).unwrap().unwrap().expose_secret(),
            TOKEN
        );
        assert!(!format!("{credential:?}").contains(TOKEN));
        store.delete(&identity).unwrap();
        assert!(store.get(&identity).unwrap().is_none());
    }
}
