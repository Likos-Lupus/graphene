use graphene_auth::{Account, AuthInteraction};
use graphene_core::{OperationHandle, Result};
use graphene_launch::LaunchSession;
use std::{future::Future, pin::Pin};

pub struct MicrosoftLoginOperation {
    pub(super) operation: OperationHandle,
    pub(super) interaction: AuthInteraction,
    pub(super) future: Pin<Box<dyn Future<Output = Result<Account>> + Send>>,
}

impl MicrosoftLoginOperation {
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }

    #[must_use]
    pub fn interaction(&self) -> &AuthInteraction {
        &self.interaction
    }

    pub async fn await_result(self) -> Result<Account> {
        self.future.await
    }
}

impl std::fmt::Debug for MicrosoftLoginOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicrosoftLoginOperation")
            .field("operation_id", &self.operation.id())
            .field("interaction", &self.interaction)
            .finish_non_exhaustive()
    }
}

pub struct AccountSessionOperation {
    pub(super) operation: OperationHandle,
    pub(super) future: Pin<Box<dyn Future<Output = Result<LaunchSession>> + Send>>,
}

impl AccountSessionOperation {
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }

    pub async fn await_result(self) -> Result<LaunchSession> {
        self.future.await
    }
}

impl std::fmt::Debug for AccountSessionOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountSessionOperation")
            .field("operation_id", &self.operation.id())
            .finish_non_exhaustive()
    }
}
