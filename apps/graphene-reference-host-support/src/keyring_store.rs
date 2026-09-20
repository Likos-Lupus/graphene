use graphene::{
    ErrorCode, ErrorKind, GrapheneError, RefreshCredential, SecretRecordIdentity, SecretStore,
};

/// Production OS credential-vault adapter implementing Graphene's `SecretStore` contract.
///
/// Platform behavior is delegated to the `keyring` v1-compatible adapter: macOS Keychain Services,
/// Windows Credential Manager, and the Secret Service on non-macOS `*nix`. There is no plaintext
/// fallback: if the backend cannot be initialized, reads and writes fail explicitly. Namespacing
/// uses the engine's stable [`SecretRecordIdentity::key`].
pub struct KeyringSecretStore {
    service: String,
}

impl KeyringSecretStore {
    /// Default host service namespace used for every credential entry.
    pub const DEFAULT_SERVICE: &'static str = "graphene";

    #[must_use]
    pub fn new() -> Self {
        Self::with_service(Self::DEFAULT_SERVICE)
    }

    #[must_use]
    pub fn with_service(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    /// Reports whether the platform credential backend initialized successfully.
    #[must_use]
    pub fn backend_available() -> bool {
        keyring::Entry::store_status().is_ok()
    }

    fn entry(&self, identity: &SecretRecordIdentity) -> Result<keyring::Entry, GrapheneError> {
        keyring::Entry::new(&self.service, &identity.key()).map_err(|_| {
            GrapheneError::new(
                ErrorCode::AuthSecretStoreUnavailable,
                ErrorKind::Authentication,
                "secure credential backend is unavailable",
            )
            .with_context("backend", "unavailable")
        })
    }
}

impl Default for KeyringSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for KeyringSecretStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyringSecretStore")
            .field("service", &self.service)
            .finish_non_exhaustive()
    }
}

impl SecretStore for KeyringSecretStore {
    fn get(
        &self,
        identity: &SecretRecordIdentity,
    ) -> Result<Option<RefreshCredential>, GrapheneError> {
        let entry = self.entry(identity)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(RefreshCredential::new(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(GrapheneError::new(
                ErrorCode::AuthSecretReadFailed,
                ErrorKind::Authentication,
                "secure credential read failed",
            )
            .with_context("backend", "read-failed")),
        }
    }

    fn put(
        &self,
        identity: &SecretRecordIdentity,
        value: &RefreshCredential,
    ) -> Result<(), GrapheneError> {
        let entry = self.entry(identity)?;
        entry.set_password(value.expose_secret()).map_err(|_| {
            GrapheneError::new(
                ErrorCode::AuthSecretWriteFailed,
                ErrorKind::Authentication,
                "secure credential write failed",
            )
            .with_context("backend", "write-failed")
        })
    }

    fn delete(&self, identity: &SecretRecordIdentity) -> Result<(), GrapheneError> {
        let entry = self.entry(identity)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(GrapheneError::new(
                ErrorCode::AuthSecretDeleteFailed,
                ErrorKind::Authentication,
                "secure credential deletion failed",
            )
            .with_context("backend", "delete-failed")),
        }
    }
}
