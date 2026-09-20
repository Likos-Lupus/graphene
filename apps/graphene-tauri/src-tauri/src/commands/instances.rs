use crate::dto::{InstanceSummary, parse_instance_id};
use crate::error::HostError;
use crate::state::TauriAppState;
use graphene::{
    CommittedInstance, RepairOptions, RepairPlan, RepairResult, VerificationMode,
    VerificationReport,
};
use tauri::State;

pub(crate) async fn list(state: &TauriAppState) -> Result<Vec<InstanceSummary>, HostError> {
    let entries = state.engine().instances().list().await?;
    Ok(entries.iter().map(InstanceSummary::from).collect())
}

pub(crate) async fn get(
    state: &TauriAppState,
    instance_id: &str,
) -> Result<CommittedInstance, HostError> {
    state
        .engine()
        .instances()
        .get(parse_instance_id(instance_id)?)
        .await
        .map_err(HostError::from)
}

pub(crate) async fn verify(
    state: &TauriAppState,
    instance_id: &str,
    full: bool,
) -> Result<VerificationReport, HostError> {
    let mode = if full {
        VerificationMode::Full
    } else {
        VerificationMode::Quick
    };
    
    let operation = state
        .engine()
        .instances()
        .verify(parse_instance_id(instance_id)?, mode);
    operation.await_result().await.map_err(HostError::from)
}

pub(crate) async fn plan_repair(
    state: &TauriAppState,
    instance_id: &str,
    full: bool,
) -> Result<RepairPlan, HostError> {
    let options = RepairOptions {
        verification_mode: if full {
            VerificationMode::Full
        } else {
            VerificationMode::Quick
        },
    };
    
    state
        .engine()
        .instances()
        .plan_repair(parse_instance_id(instance_id)?, options)
        .await
        .map_err(HostError::from)
}

pub(crate) async fn repair(
    state: &TauriAppState,
    instance_id: &str,
    full: bool,
) -> Result<RepairResult, HostError> {
    let plan = plan_repair(state, instance_id, full).await?;
    let operation = state.engine().instances().execute_repair(plan);
    operation.await_result().await.map_err(HostError::from)
}

#[tauri::command]
pub async fn instances_list(
    state: State<'_, TauriAppState>,
) -> Result<Vec<InstanceSummary>, HostError> {
    list(state.inner()).await
}

#[tauri::command]
pub async fn instance_get(
    state: State<'_, TauriAppState>,
    instance_id: String,
) -> Result<CommittedInstance, HostError> {
    get(state.inner(), &instance_id).await
}

#[tauri::command]
pub async fn instance_verify(
    state: State<'_, TauriAppState>,
    instance_id: String,
    full: bool,
) -> Result<VerificationReport, HostError> {
    verify(state.inner(), &instance_id, full).await
}

#[tauri::command]
pub async fn instance_plan_repair(
    state: State<'_, TauriAppState>,
    instance_id: String,
    full: bool,
) -> Result<RepairPlan, HostError> {
    plan_repair(state.inner(), &instance_id, full).await
}

#[tauri::command]
pub async fn instance_repair(
    state: State<'_, TauriAppState>,
    instance_id: String,
    full: bool,
) -> Result<RepairResult, HostError> {
    repair(state.inner(), &instance_id, full).await
}
