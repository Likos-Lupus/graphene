use crate::commands::parse_loader;
use crate::error::HostError;
use crate::state::TauriAppState;
use graphene::{
    CommittedInstance, ComponentInstallRequest, InstallPlanOperation, InstallRequest,
    LoaderSelection, LoaderVersion, LoaderVersionSelector,
};
use tauri::State;

async fn run_install(
    state: &TauriAppState,
    operation: InstallPlanOperation,
) -> Result<CommittedInstance, HostError> {
    let plan = operation.await_result().await?;
    let execute = state.engine().install().execute(plan);
    execute.await_result().await.map_err(HostError::from)
}

pub(crate) async fn vanilla(
    state: &TauriAppState,
    minecraft: String,
    name: String,
) -> Result<CommittedInstance, HostError> {
    let request = InstallRequest::new(name, minecraft)?;
    let operation = state.engine().install().plan(request);
    run_install(state, operation).await
}

pub(crate) async fn run_loader(
    state: &TauriAppState,
    minecraft: String,
    loader: String,
    loader_version: Option<String>,
    name: String,
) -> Result<CommittedInstance, HostError> {
    let selector = match loader_version {
        Some(value) => LoaderVersionSelector::Exact(LoaderVersion::new(value)?),
        None => LoaderVersionSelector::LatestStable,
    };
    let selection = LoaderSelection {
        kind: parse_loader(&loader)?,
        version: selector,
    };
    let request = ComponentInstallRequest::new(name, minecraft, selection)?;
    let operation = state.engine().install().plan_components(request);

    run_install(state, operation).await
}

#[tauri::command]
pub async fn install_vanilla(
    state: State<'_, TauriAppState>,
    minecraft: String,
    name: String,
) -> Result<CommittedInstance, HostError> {
    vanilla(state.inner(), minecraft, name).await
}

#[tauri::command]
pub async fn install_loader(
    state: State<'_, TauriAppState>,
    minecraft: String,
    loader: String,
    loader_version: Option<String>,
    name: String,
) -> Result<CommittedInstance, HostError> {
    run_loader(state.inner(), minecraft, loader, loader_version, name).await
}
