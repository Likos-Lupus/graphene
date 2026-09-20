use crate::dto::parse_instance_id;
use crate::error::HostError;
use crate::state::TauriAppState;
use graphene::{JavaRuntime, ManagedJavaRuntime};
use tauri::State;

pub(crate) async fn list(state: &TauriAppState) -> Result<Vec<ManagedJavaRuntime>, HostError> {
    state
        .engine()
        .java()
        .managed_runtimes()
        .await
        .map_err(HostError::from)
}

pub(crate) async fn ensure(
    state: &TauriAppState,
    instance_id: &str,
) -> Result<JavaRuntime, HostError> {
    let operation = state
        .engine()
        .java()
        .ensure_for_instance(parse_instance_id(instance_id)?, None);
    operation.await_result().await.map_err(HostError::from)
}

#[tauri::command]
pub async fn java_list(
    state: State<'_, TauriAppState>,
) -> Result<Vec<ManagedJavaRuntime>, HostError> {
    list(state.inner()).await
}

#[tauri::command]
pub async fn java_ensure(
    state: State<'_, TauriAppState>,
    instance_id: String,
) -> Result<JavaRuntime, HostError> {
    ensure(state.inner(), &instance_id).await
}
