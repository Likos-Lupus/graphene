use crate::error::HostError;
use crate::state::TauriAppState;
use graphene::{OperationId, OperationSnapshot};
use graphene_reference_host_support::OperationSummary;
use tauri::State;

pub(crate) fn list(state: &TauriAppState) -> Vec<OperationSummary> {
    state.operations().list()
}

pub(crate) fn cancel(state: &TauriAppState, operation_id: &str) -> Result<(), HostError> {
    let id = parse_operation_id(operation_id)?;
    state.operations().cancel(id).map_err(HostError::from)
}

pub(crate) fn snapshot(
    state: &TauriAppState,
    operation_id: &str,
) -> Result<Option<OperationSnapshot>, HostError> {
    let id = parse_operation_id(operation_id)?;
    Ok(state.operations().snapshot(id))
}

fn parse_operation_id(value: &str) -> Result<OperationId, HostError> {
    value
        .parse::<OperationId>()
        .map_err(|_| HostError::invalid("operation_id", value))
}

#[tauri::command]
pub async fn operations_list(
    state: State<'_, TauriAppState>,
) -> Result<Vec<OperationSummary>, HostError> {
    Ok(list(state.inner()))
}

#[tauri::command]
pub async fn operations_cancel(
    state: State<'_, TauriAppState>,
    operation_id: String,
) -> Result<(), HostError> {
    cancel(state.inner(), &operation_id)
}

#[tauri::command]
pub async fn operations_snapshot(
    state: State<'_, TauriAppState>,
    operation_id: String,
) -> Result<Option<OperationSnapshot>, HostError> {
    snapshot(state.inner(), &operation_id)
}
