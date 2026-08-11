use super::{
    conversion::to_launch_session,
    error::{account_not_found, auth_error, cancelled},
    operation::{AccountSessionOperation, MicrosoftLoginOperation},
};
use crate::{context::ServiceContext, operation_lifecycle};
use graphene_auth::{
    Account, AccountKind, AccountState, AuthChallenge, AuthProvider, OfflineAccountSpec,
    RefreshDisposition, SecretRecordIdentity, classify_refresh_error, create_offline_account,
    offline_auth_session,
};
use graphene_core::{AccountId, ErrorCode, ErrorKind, OperationController, Result};
use graphene_launch::LaunchSession;
use std::sync::Arc;

#[derive(Clone)]
pub struct AccountService {
    pub(super) context: Arc<ServiceContext>,
}

impl std::fmt::Debug for AccountService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountService").finish_non_exhaustive()
    }
}

impl AccountService {
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        Self { context }
    }

    pub async fn list(&self) -> Result<Vec<Account>> {
        let accounts = self.repository_list().await?;
        let mut reconciled = Vec::with_capacity(accounts.len());

        for account in accounts {
            reconciled.push(self.reconcile(account).await?);
        }
        Ok(reconciled)
    }

    pub async fn get(&self, account_id: AccountId) -> Result<Option<Account>> {
        match self.repository_get(account_id).await? {
            Some(account) => Ok(Some(self.reconcile(account).await?)),
            None => Ok(None),
        }
    }

    pub async fn create_offline(&self, spec: OfflineAccountSpec) -> Result<Account> {
        let account = create_offline_account(spec)?;
        self.repository_put(account.clone()).await?;
        Ok(account)
    }

    pub async fn begin_microsoft_login(&self) -> Result<MicrosoftLoginOperation> {
        self.begin_login(None).await
    }

    pub async fn reauthenticate(&self, account_id: AccountId) -> Result<MicrosoftLoginOperation> {
        let account = self
            .repository_get(account_id)
            .await?
            .ok_or_else(|| account_not_found(account_id))?;
        if account.kind != AccountKind::Microsoft {
            return Err(auth_error(
                ErrorCode::AuthAccountStateInvalid,
                "offline accounts do not require provider reauthentication",
            ));
        }
        self.begin_login(Some(account_id)).await
    }

    pub fn launch_session(&self, account_id: AccountId) -> AccountSessionOperation {
        let controller = self.context.operations.create("account-launch-session");
        let operation = controller.handle();
        let service = self.clone();
        let future =
            Box::pin(async move { service.run_launch_session(account_id, controller).await });
        AccountSessionOperation { operation, future }
    }

    pub async fn remove(&self, account_id: AccountId) -> Result<()> {
        let gate = self.context.account_gate(account_id);
        let _guard = gate.lock().await;
        let account = self
            .repository_get(account_id)
            .await?
            .ok_or_else(|| account_not_found(account_id))?;
        if account.kind == AccountKind::Microsoft {
            self.secret_delete(SecretRecordIdentity::microsoft_refresh(account_id))
                .await?;
        }
        self.repository_delete(account_id).await
    }

    async fn begin_login(&self, target: Option<AccountId>) -> Result<MicrosoftLoginOperation> {
        let provider = self.context.auth_provider.clone().ok_or_else(|| {
            auth_error(
                ErrorCode::AuthProviderUnavailable,
                "Microsoft authentication is not configured",
            )
        })?;
        let controller = self.context.operations.create(if target.is_some() {
            "account-reauthenticate"
        } else {
            "account-microsoft-login"
        });

        operation_lifecycle::start(&controller)?;
        let challenge = match provider.begin(&controller).await {
            Ok(challenge) => challenge,
            Err(error) => {
                if error.is_cancelled() {
                    let _ = controller.cancelled();
                } else {
                    let _ = controller.fail(error.summary());
                }
                return Err(error);
            }
        };
        let interaction = challenge.interaction.clone();
        let operation = controller.handle();
        let service = self.clone();
        let future = Box::pin(async move {
            let result = service
                .complete_login(target, provider, challenge, &controller)
                .await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::AuthCancelled,
                ErrorKind::Authentication,
                "authentication operation was cancelled",
            )
        });

        Ok(MicrosoftLoginOperation {
            operation,
            interaction,
            future,
        })
    }

    async fn complete_login(
        &self,
        target: Option<AccountId>,
        provider: Arc<dyn AuthProvider>,
        challenge: AuthChallenge,
        controller: &OperationController,
    ) -> Result<Account> {
        let session = provider.complete(challenge, controller).await?;
        if session.kind != AccountKind::Microsoft {
            return Err(auth_error(
                ErrorCode::AuthAccountStateInvalid,
                "Microsoft adapter returned a non-Microsoft session",
            ));
        }

        let refresh = session.refresh_credential.clone().ok_or_else(|| {
            auth_error(
                ErrorCode::AuthSecretWriteFailed,
                "Microsoft login did not produce a refresh credential",
            )
        })?;
        controller.set_stage("persist_refresh_credential")?;

        if controller.is_cancelled() {
            return Err(cancelled());
        }

        if let Some(account_id) = target {
            let gate = self.context.account_gate(account_id);
            let token = controller.cancellation_token();
            let _guard = tokio::select! {
                () = token.cancelled() => return Err(cancelled()),
                guard = gate.lock() => guard,
            };
            let mut account = self
                .repository_get(account_id)
                .await?
                .ok_or_else(|| account_not_found(account_id))?;

            if account.kind != AccountKind::Microsoft
                || account.profile.minecraft_uuid != session.profile.minecraft_uuid
            {
                return Err(auth_error(
                    ErrorCode::AuthAccountStateInvalid,
                    "reauthentication profile identity does not match the existing account",
                ));
            }

            let identity = SecretRecordIdentity::microsoft_refresh(account_id);
            let previous = self.secret_get(identity.clone()).await?;

            self.secret_put(identity.clone(), refresh).await?;
            if controller.is_cancelled() {
                match previous {
                    Some(previous) => self.secret_put(identity, previous).await?,
                    None => self.secret_delete(identity).await?,
                }
                return Err(cancelled());
            }

            controller.set_stage("persist_account")?;
            account.profile = session.profile;
            account.state = AccountState::Ready;
            if let Err(error) = self.repository_put(account.clone()).await {
                match previous {
                    Some(previous) => {
                        let _ = self.secret_put(identity, previous).await;
                    }
                    None => {
                        let _ = self.secret_delete(identity).await;
                    }
                }
                return Err(error);
            }

            controller.set_stage("complete")?;
            return Ok(account);
        }

        let account = Account {
            id: AccountId::new(),
            kind: AccountKind::Microsoft,
            profile: session.profile,
            state: AccountState::Ready,
        };
        account.validate()?;
        let identity = SecretRecordIdentity::microsoft_refresh(account.id);
        self.secret_put(identity.clone(), refresh).await?;

        if controller.is_cancelled() {
            let _ = self.secret_delete(identity.clone()).await;
            return Err(cancelled());
        }

        controller.set_stage("persist_account")?;
        if let Err(error) = self.repository_put(account.clone()).await {
            if let Err(cleanup) = self.secret_delete(identity).await {
                return Err(auth_error(
                    ErrorCode::AuthPersistenceFailed,
                    "public account persistence failed and secret cleanup also failed",
                )
                .with_context("account_id", account.id.to_string())
                .with_context("recovery", "orphan_secret_cleanup_required")
                .with_source(cleanup));
            }

            return Err(error);
        }

        controller.set_stage("complete")?;
        Ok(account)
    }

    async fn run_launch_session(
        &self,
        account_id: AccountId,
        controller: OperationController,
    ) -> Result<LaunchSession> {
        operation_lifecycle::start(&controller)?;
        let result = self.launch_session_inner(account_id, &controller).await;
        operation_lifecycle::finish(
            &controller,
            result,
            ErrorCode::AuthCancelled,
            ErrorKind::Authentication,
            "authentication operation was cancelled",
        )
    }

    async fn launch_session_inner(
        &self,
        account_id: AccountId,
        controller: &OperationController,
    ) -> Result<LaunchSession> {
        let gate = self.context.account_gate(account_id);
        let token = controller.cancellation_token();
        let _guard = tokio::select! {
            () = token.cancelled() => return Err(cancelled()),
            guard = gate.lock() => guard,
        };
        controller.set_stage("load_account")?;
        let mut account = self
            .repository_get(account_id)
            .await?
            .ok_or_else(|| account_not_found(account_id))?;

        if account.kind == AccountKind::Offline {
            return to_launch_session(offline_auth_session(&account)?);
        }

        if account.state != AccountState::Ready {
            return Err(auth_error(
                ErrorCode::AuthInteractionRequired,
                "account requires reauthentication before launch",
            ));
        }

        controller.set_stage("load_refresh_credential")?;
        let identity = SecretRecordIdentity::microsoft_refresh(account_id);
        let credential = match self.secret_get(identity).await? {
            Some(value) => value,
            None => {
                account.state = AccountState::RequiresReauthentication;
                self.repository_put(account).await?;
                return Err(auth_error(
                    ErrorCode::AuthInteractionRequired,
                    "account refresh credential is missing",
                ));
            }
        };
        let provider = self.context.auth_provider.clone().ok_or_else(|| {
            auth_error(
                ErrorCode::AuthProviderUnavailable,
                "Microsoft authentication is not configured",
            )
        })?;
        let session = match provider.refresh(&account, &credential, controller).await {
            Ok(session) => session,
            Err(error) => {
                if classify_refresh_error(&error) == RefreshDisposition::RequireReauthentication {
                    account.state = AccountState::RequiresReauthentication;
                    self.repository_put(account).await?;
                }
                return Err(error);
            }
        };

        if session.kind != AccountKind::Microsoft
            || session.profile.minecraft_uuid != account.profile.minecraft_uuid
        {
            account.state = AccountState::RequiresReauthentication;
            self.repository_put(account).await?;
            return Err(auth_error(
                ErrorCode::AuthAccountStateInvalid,
                "refreshed profile identity does not match persisted account",
            ));
        }

        if let Some(rotated) = session.refresh_credential.clone()
            && rotated != credential
        {
            controller.set_stage("persist_refresh_credential")?;
            if controller.is_cancelled() {
                return Err(cancelled());
            }

            let identity = SecretRecordIdentity::microsoft_refresh(account_id);
            self.secret_put(identity, rotated).await?;
            if controller.is_cancelled() {
                // The provider may have invalidated the old credential as part of rotation.
                // Once the replacement is durably stored, keeping it is the recoverable state.
                return Err(cancelled());
            }
        }

        if controller.is_cancelled() {
            return Err(cancelled());
        }

        account.profile = session.profile.clone();
        account.state = AccountState::Ready;
        controller.set_stage("persist_account")?;
        self.repository_put(account).await?;
        controller.set_stage("complete")?;
        to_launch_session(session)
    }

    async fn reconcile(&self, mut account: Account) -> Result<Account> {
        if account.kind != AccountKind::Microsoft || account.state != AccountState::Ready {
            return Ok(account);
        }

        match self
            .secret_get(SecretRecordIdentity::microsoft_refresh(account.id))
            .await
        {
            Ok(Some(_)) => Ok(account),
            Ok(None) => {
                account.state = AccountState::RequiresReauthentication;
                self.repository_put(account.clone()).await?;
                Ok(account)
            }
            Err(error) if error.code == ErrorCode::AuthSecretStoreUnavailable => {
                account.state = AccountState::SecretStoreUnavailable;
                Ok(account)
            }
            Err(error) => Err(error),
        }
    }
}
