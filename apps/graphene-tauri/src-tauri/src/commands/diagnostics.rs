use crate::dto::parse_instance_id;
use crate::error::HostError;
use crate::state::TauriAppState;
use graphene::{DiagnosticReport, DiagnosticRequest, ProcessExitEvidence};
use tauri::State;

pub(crate) async fn diagnose(
    state: &TauriAppState,
    instance_id: &str,
    mode: &str,
    exit_code: Option<i32>,
) -> Result<DiagnosticReport, HostError> {
    let request = match mode {
        "preflight" => DiagnosticRequest::preflight(),
        "crash" => DiagnosticRequest::crash(),
        "full" => DiagnosticRequest::full(),
        _ => return Err(HostError::invalid("mode", mode)),
    };
    let request = match exit_code {
        Some(code) => {
            request.with_process_exit(ProcessExitEvidence::new(Some(code), code == 0, false))
        }
        None => request,
    };
    let operation = state
        .engine()
        .diagnostics()
        .analyze(parse_instance_id(instance_id)?, request);
    operation.await_result().await.map_err(HostError::from)
}

#[tauri::command]
pub async fn diagnostics_analyze(
    state: State<'_, TauriAppState>,
    instance_id: String,
    mode: String,
    exit_code: Option<i32>,
) -> Result<DiagnosticReport, HostError> {
    diagnose(state.inner(), &instance_id, &mode, exit_code).await
}
