use crate::dto::{EngineInfo, OperationHandleView};
use crate::error::HostError;
use crate::events;
use crate::state::TauriAppState;
use graphene::OperationId;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, State};

pub(crate) fn info(state: &TauriAppState) -> EngineInfo {
    let platform = state.engine().platform();
    EngineInfo {
        data_root: state.data_root().display().to_string(),
        os: format!("{:?}", platform.os),
        architecture: format!("{:?}", platform.architecture),
    }
}

/// Registers and drives a deterministic synthetic operation without needing an `AppHandle`. The
/// command wrapper adds the Tauri event bridge on top so the logic stays unit-testable.
pub(crate) fn register_synthetic(state: &TauriAppState, steps: u64, delay_ms: u64) -> OperationId {
    let operation =
        state
            .engine()
            .operations()
            .synthetic(None, steps, Duration::from_millis(delay_ms));
    let handle = operation.operation();
    let id = state.operations().register(handle);
    
    tauri::async_runtime::spawn(async move {
        let _ = operation.await_result().await;
    });
    
    id
}

#[tauri::command]
pub async fn engine_info(state: State<'_, TauriAppState>) -> Result<EngineInfo, HostError> {
    Ok(info(state.inner()))
}

#[tauri::command]
pub async fn engine_start_synthetic(
    app: AppHandle,
    state: State<'_, TauriAppState>,
    steps: u64,
    delay_ms: u64,
) -> Result<OperationHandleView, HostError> {
    let id = register_synthetic(state.inner(), steps, delay_ms);
    events::forward_operation(app, Arc::clone(state.operations()), id);
    
    Ok(OperationHandleView {
        operation_id: id.to_string(),
    })
}
