pub mod account;
pub mod content;
pub mod diagnose;
pub mod engine;
pub mod install;
pub mod instance;
pub mod java;
pub mod launch;
pub mod modpack;

use crate::context::AppContext;
use crate::error::cancelled;
use graphene::{
    AccountId, ContentProviderId, ErrorCode, ErrorKind, GrapheneError, InstanceId, LoaderKind,
    LoaderSelection, LoaderVersion, LoaderVersionSelector, OperationEventKind, OperationHandle,
};
use std::future::Future;

/// Awaits an operation future while rendering bounded progress and honoring Ctrl+C.
///
/// On interrupt the handle is cancelled and the operation is allowed to finish cooperatively so it
/// never leaves a half-terminal operation behind.
pub async fn await_operation<T, F>(
    handle: OperationHandle,
    future: F,
    show_progress: bool,
) -> Result<T, GrapheneError>
where
    F: Future<Output = Result<T, GrapheneError>>,
{
    let progress = if show_progress {
        Some(tokio::spawn(forward_progress(handle.clone())))
    } else {
        None
    };

    tokio::pin!(future);
    let outcome = tokio::select! {
        result = &mut future => result,
        signal = tokio::signal::ctrl_c() => {
            handle.cancel();
            match signal {
                Ok(()) => {
                    let _ = future.await;
                    Err(cancelled())
                }

                Err(source) => Err(GrapheneError::new(
                    ErrorCode::OperationCancelled,
                    ErrorKind::Cancelled,
                    "interrupt handler failed",
                )
                .with_source(source)),
            }
        }
    };

    if let Some(task) = progress {
        let _ = task.await;
    }

    outcome
}

async fn forward_progress(handle: OperationHandle) {
    let stream = handle.subscribe();
    while let Some(event) = stream.next().await {
        match &event.kind {
            OperationEventKind::StageChanged { stage } => eprintln!("stage: {stage}"),
            OperationEventKind::Progress { progress } => eprintln!("progress: {progress:?}"),
            OperationEventKind::StateChanged { state } => eprintln!("state: {state:?}"),
            _ => {}
        }
    }
}

#[must_use]
pub fn progress_enabled(context: &AppContext) -> bool {
    !context.output.is_json()
}

pub fn parse_instance(value: &str) -> Result<InstanceId, GrapheneError> {
    value
        .parse::<InstanceId>()
        .map_err(|source| invalid("instance_id", value, source))
}

pub fn parse_account(value: &str) -> Result<AccountId, GrapheneError> {
    value
        .parse::<AccountId>()
        .map_err(|source| invalid("account_id", value, source))
}

#[must_use]
pub fn provider_id(provider: crate::cli::ProviderArg) -> ContentProviderId {
    let id = match provider {
        crate::cli::ProviderArg::Modrinth => ContentProviderId::MODRINTH,
        crate::cli::ProviderArg::Curseforge => ContentProviderId::CURSEFORGE,
    };
    ContentProviderId::new(id).expect("static provider id is valid")
}

#[must_use]
pub fn loader_kind(loader: crate::cli::LoaderArg) -> LoaderKind {
    match loader {
        crate::cli::LoaderArg::Fabric => LoaderKind::Fabric,
        crate::cli::LoaderArg::Forge => LoaderKind::Forge,
        crate::cli::LoaderArg::NeoForge => LoaderKind::NeoForge,
    }
}

pub fn loader_selection(
    loader: crate::cli::LoaderArg,
    version: Option<&str>,
) -> Result<LoaderSelection, GrapheneError> {
    let selector = match version {
        Some(value) => LoaderVersionSelector::Exact(LoaderVersion::new(value)?),
        None => LoaderVersionSelector::LatestStable,
    };

    Ok(LoaderSelection {
        kind: loader_kind(loader),
        version: selector,
    })
}

fn invalid<E>(field: &str, value: &str, source: E) -> GrapheneError
where
    E: std::error::Error + Send + Sync + 'static,
{
    GrapheneError::new(
        ErrorCode::ConfigInvalid,
        ErrorKind::Configuration,
        format!("invalid {field}"),
    )
    .with_context(field, value)
    .with_source(source)
}
