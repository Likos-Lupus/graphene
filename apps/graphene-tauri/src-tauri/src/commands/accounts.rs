use crate::dto::{AccountSummary, AuthInteractionView, parse_account_id};
use crate::error::HostError;
use crate::events;
use crate::state::TauriAppState;
use graphene::OfflineAccountSpec;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

pub(crate) async fn list(state: &TauriAppState) -> Result<Vec<AccountSummary>, HostError> {
    let accounts = state.engine().accounts().list().await?;
    Ok(accounts.iter().map(AccountSummary::from).collect())
}

pub(crate) async fn offline_add(
    state: &TauriAppState,
    name: &str,
) -> Result<AccountSummary, HostError> {
    let account = state
        .engine()
        .accounts()
        .create_offline(OfflineAccountSpec::new(name))
        .await?;
    Ok(AccountSummary::from(&account))
}

pub(crate) async fn remove(state: &TauriAppState, account_id: &str) -> Result<(), HostError> {
    state
        .engine()
        .accounts()
        .remove(parse_account_id(account_id)?)
        .await
        .map_err(HostError::from)
}

#[tauri::command]
pub async fn accounts_list(
    state: State<'_, TauriAppState>,
) -> Result<Vec<AccountSummary>, HostError> {
    list(state.inner()).await
}

#[tauri::command]
pub async fn account_offline_add(
    state: State<'_, TauriAppState>,
    name: String,
) -> Result<AccountSummary, HostError> {
    offline_add(state.inner(), &name).await
}

#[tauri::command]
pub async fn account_remove(
    state: State<'_, TauriAppState>,
    account_id: String,
) -> Result<(), HostError> {
    remove(state.inner(), &account_id).await
}

#[tauri::command]
pub async fn account_begin_microsoft_login(
    app: AppHandle,
    state: State<'_, TauriAppState>,
) -> Result<AuthInteractionView, HostError> {
    let operation = state.engine().accounts().begin_microsoft_login().await?;
    let interaction = AuthInteractionView::from_interaction(
        operation.operation().id().to_string(),
        operation.interaction(),
    );

    let id = state.operations().register(operation.operation());
    events::forward_operation(app.clone(), Arc::clone(state.operations()), id);

    let result_app = app;
    tauri::async_runtime::spawn(async move {
        match operation.await_result().await {
            Ok(account) => {
                let _ = result_app.emit(
                    events::AUTH_RESULT_EVENT,
                    serde_json::json!({ "ok": true, "account": AccountSummary::from(&account) }),
                );
            }

            Err(error) => {
                let _ = result_app.emit(
                    events::AUTH_RESULT_EVENT,
                    serde_json::json!({ "ok": false, "error": HostError::from(error) }),
                );
            }
        }
    });

    Ok(interaction)
}
