use super::{error::auth_error, service::AccountService};
use graphene_auth::{Account, RefreshCredential, SecretRecordIdentity};
use graphene_core::{AccountId, ErrorCode, GrapheneError, Result};
use std::sync::Arc;

impl AccountService {
    pub(super) async fn repository_list(&self) -> Result<Vec<Account>> {
        let repository = Arc::clone(&self.context.account_repository);
        spawn_blocking_auth(move || repository.list())
            .await
            .map_err(map_persistence_error)
    }

    pub(super) async fn repository_get(&self, id: AccountId) -> Result<Option<Account>> {
        let repository = Arc::clone(&self.context.account_repository);
        spawn_blocking_auth(move || repository.get(id))
            .await
            .map_err(map_persistence_error)
    }

    pub(super) async fn repository_put(&self, account: Account) -> Result<()> {
        let repository = Arc::clone(&self.context.account_repository);
        spawn_blocking_auth(move || repository.put(&account))
            .await
            .map_err(map_persistence_error)
    }

    pub(super) async fn repository_delete(&self, id: AccountId) -> Result<()> {
        let repository = Arc::clone(&self.context.account_repository);
        spawn_blocking_auth(move || repository.delete(id))
            .await
            .map_err(map_persistence_error)
    }

    pub(super) async fn secret_get(
        &self,
        identity: SecretRecordIdentity,
    ) -> Result<Option<RefreshCredential>> {
        let store = Arc::clone(&self.context.secret_store);
        spawn_blocking_auth(move || store.get(&identity))
            .await
            .map_err(|error| {
                map_secret_error(
                    error,
                    ErrorCode::AuthSecretReadFailed,
                    "secure credential read failed",
                )
            })
    }

    pub(super) async fn secret_put(
        &self,
        identity: SecretRecordIdentity,
        value: RefreshCredential,
    ) -> Result<()> {
        let store = Arc::clone(&self.context.secret_store);
        spawn_blocking_auth(move || store.put(&identity, &value))
            .await
            .map_err(|error| {
                map_secret_error(
                    error,
                    ErrorCode::AuthSecretWriteFailed,
                    "secure credential write failed",
                )
            })
    }

    pub(super) async fn secret_delete(&self, identity: SecretRecordIdentity) -> Result<()> {
        let store = Arc::clone(&self.context.secret_store);
        spawn_blocking_auth(move || store.delete(&identity))
            .await
            .map_err(|error| {
                map_secret_error(
                    error,
                    ErrorCode::AuthSecretDeleteFailed,
                    "secure credential deletion failed",
                )
            })
    }
}

async fn spawn_blocking_auth<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work).await.map_err(|source| {
        auth_error(
            ErrorCode::AuthPersistenceFailed,
            "blocking authentication adapter task failed",
        )
        .with_source(source)
    })?
}

fn map_persistence_error(error: GrapheneError) -> GrapheneError {
    if error.code == ErrorCode::AuthPersistenceFailed {
        error
    } else {
        auth_error(
            ErrorCode::AuthPersistenceFailed,
            "account persistence operation failed",
        )
        .with_source(error)
    }
}

fn map_secret_error(error: GrapheneError, code: ErrorCode, message: &'static str) -> GrapheneError {
    if error.code == ErrorCode::AuthSecretStoreUnavailable || error.code == code {
        error
    } else {
        auth_error(code, message).with_source(error)
    }
}
