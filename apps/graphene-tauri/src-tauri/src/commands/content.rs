use crate::dto::{parse_instance_id, parse_provider_id};
use crate::error::HostError;
use crate::state::TauriAppState;
use graphene::{ContentSearchPage, ContentSearchQuery, LocalContentInventory};
use tauri::State;

pub(crate) async fn scan(
    state: &TauriAppState,
    instance_id: &str,
    hashes: bool,
) -> Result<LocalContentInventory, HostError> {
    let operation = state
        .engine()
        .content()
        .scan(parse_instance_id(instance_id)?, hashes);
    operation.await_result().await.map_err(HostError::from)
}

pub(crate) async fn search(
    state: &TauriAppState,
    provider: &str,
    query: String,
    minecraft_version: Option<String>,
    loader: Option<String>,
    limit: u32,
) -> Result<ContentSearchPage, HostError> {
    let loader = match loader.as_deref() {
        Some(value) => Some(crate::commands::parse_loader(value)?),
        None => None,
    };
    let query = ContentSearchQuery {
        query,
        minecraft_version,
        loader,
        offset: 0,
        limit,
    };
    let operation = state
        .engine()
        .content()
        .search(&parse_provider_id(provider)?, &query);
    operation.await_result().await.map_err(HostError::from)
}

#[tauri::command]
pub async fn content_scan(
    state: State<'_, TauriAppState>,
    instance_id: String,
    hashes: bool,
) -> Result<LocalContentInventory, HostError> {
    scan(state.inner(), &instance_id, hashes).await
}

#[tauri::command]
pub async fn content_search(
    state: State<'_, TauriAppState>,
    provider: String,
    query: String,
    minecraft_version: Option<String>,
    loader: Option<String>,
    limit: u32,
) -> Result<ContentSearchPage, HostError> {
    search(
        state.inner(),
        &provider,
        query,
        minecraft_version,
        loader,
        limit,
    )
    .await
}
