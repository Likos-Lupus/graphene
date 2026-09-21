use crate::dto::{RunHandleView, RunSummaryView, parse_account_id, parse_instance_id};
use crate::error::HostError;
use crate::events;
use crate::state::TauriAppState;
use graphene::{LaunchRequest, RedactedLaunchPlan};
use std::sync::Arc;
use tauri::{AppHandle, State};

pub(crate) async fn plan(
    state: &TauriAppState,
    instance_id: &str,
    account_id: &str,
) -> Result<RedactedLaunchPlan, HostError> {
    let session = session(state, account_id).await?;
    let request = LaunchRequest::new(parse_instance_id(instance_id)?, session);
    let plan = state.engine().launch().plan(request).await?;
    Ok(plan.redacted_snapshot(state.data_root()))
}

pub(crate) async fn run(
    app: &AppHandle,
    state: &TauriAppState,
    instance_id: &str,
    account_id: &str,
) -> Result<RunHandleView, HostError> {
    let session = session(state, account_id).await?;
    let request = LaunchRequest::new(parse_instance_id(instance_id)?, session);
    let plan = state.engine().launch().plan(request).await?;
    let game = state.engine().launch().execute(plan)?;
    let pid = game.pid();
    let run_id = state.runs().insert(Arc::new(game));

    events::forward_run(app.clone(), Arc::clone(state.runs()), run_id.clone());

    Ok(RunHandleView { run_id, pid })
}

async fn session(
    state: &TauriAppState,
    account_id: &str,
) -> Result<graphene::LaunchSession, HostError> {
    let account = parse_account_id(account_id)?;
    state
        .engine()
        .accounts()
        .launch_session(account)
        .await_result()
        .await
        .map_err(HostError::from)
}

pub(crate) fn runs(state: &TauriAppState) -> Vec<RunSummaryView> {
    state
        .runs()
        .list()
        .into_iter()
        .map(RunSummaryView::from)
        .collect()
}

pub(crate) async fn kill_run(state: &TauriAppState, run_id: &str) -> Result<(), HostError> {
    state.runs().kill(run_id).await.map_err(HostError::from)
}

#[tauri::command]
pub async fn launch_plan(
    state: State<'_, TauriAppState>,
    instance_id: String,
    account_id: String,
) -> Result<RedactedLaunchPlan, HostError> {
    plan(state.inner(), &instance_id, &account_id).await
}

#[tauri::command]
pub async fn launch_run(
    app: AppHandle,
    state: State<'_, TauriAppState>,
    instance_id: String,
    account_id: String,
) -> Result<RunHandleView, HostError> {
    run(&app, state.inner(), &instance_id, &account_id).await
}

#[tauri::command]
pub async fn launch_kill(state: State<'_, TauriAppState>, run_id: String) -> Result<(), HostError> {
    kill_run(state.inner(), &run_id).await
}

#[tauri::command]
pub async fn launch_kill_pid(pid: u32) -> Result<(), HostError> {
    #[cfg(unix)]
    {
        // SAFETY: `pid` is a plain integer; `kill` has no memory-safety impact.
        let result = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        if result == 0 {
            Ok(())
        } else {
            Err(HostError::invalid("pid", &pid.to_string()))
        }
    }

    #[cfg(not(unix))]
    {
        let _ = pid;
        Err(HostError {
            code: "PLATFORM_UNSUPPORTED".to_owned(),
            kind: "Platform".to_owned(),
            message: "launch kill is unsupported on this platform".to_owned(),
            context: std::collections::BTreeMap::new(),
        })
    }
}

#[tauri::command]
pub async fn runs_list(state: State<'_, TauriAppState>) -> Result<Vec<RunSummaryView>, HostError> {
    Ok(runs(state.inner()))
}
