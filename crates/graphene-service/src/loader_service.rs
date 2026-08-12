use crate::{context::ServiceContext, operation_lifecycle};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, OperationHandle, Result};
use graphene_minecraft::{
    LoaderKind, LoaderSupport, LoaderVersionSelector, LoaderVersionSummary, MinecraftVersionId,
    ResolvedLoader,
};
use std::{future::Future, pin::Pin, sync::Arc};

#[derive(Clone)]
pub struct LoaderService {
    context: Arc<ServiceContext>,
}

impl std::fmt::Debug for LoaderService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoaderService").finish_non_exhaustive()
    }
}

impl LoaderService {
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        Self { context }
    }

    #[must_use]
    pub fn versions(
        &self,
        kind: LoaderKind,
        minecraft: MinecraftVersionId,
    ) -> LoaderVersionsOperation {
        let controller = self.context.operations.create("loader-versions");
        let operation = controller.handle();
        let registry = self.context.loader_registry.clone();
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = async {
                controller.set_stage("resolve-provider")?;
                let provider = registry.provider(kind)?;
                controller.set_stage("query-versions")?;
                provider.list_versions(&minecraft, &controller).await
            }
            .await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::OperationCancelled,
                ErrorKind::Cancelled,
                "loader version query was cancelled",
            )
        });
        LoaderVersionsOperation { operation, future }
    }

    #[must_use]
    pub fn resolve(
        &self,
        kind: LoaderKind,
        minecraft: MinecraftVersionId,
        selector: LoaderVersionSelector,
    ) -> LoaderResolveOperation {
        let controller = self.context.operations.create("loader-resolve");
        let operation = controller.handle();
        let registry = self.context.loader_registry.clone();
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = async {
                controller.set_stage("resolve-provider")?;
                let provider = registry.provider(kind)?;
                let exact = match selector {
                    LoaderVersionSelector::Exact(version) => version,
                    LoaderVersionSelector::LatestStable => {
                        controller.set_stage("query-versions")?;
                        provider
                            .list_versions(&minecraft, &controller)
                            .await?
                            .into_iter()
                            .find(|candidate| candidate.stable != Some(false))
                            .map(|candidate| candidate.version)
                            .ok_or_else(|| {
                                selector_error(kind, "no stable loader release is available")
                            })?
                    }
                    LoaderVersionSelector::Recommended => {
                        controller.set_stage("query-versions")?;
                        provider
                            .list_versions(&minecraft, &controller)
                            .await?
                            .into_iter()
                            .find(|candidate| candidate.recommended == Some(true))
                            .map(|candidate| candidate.version)
                            .ok_or_else(|| {
                                selector_error(kind, "no recommended loader release is available")
                            })?
                    }
                    _ => {
                        return Err(selector_error(
                            kind,
                            "loader selector variant is unsupported",
                        ));
                    }
                };
                controller.set_stage("resolve-exact")?;
                provider
                    .resolve_exact(&minecraft, &exact, &controller)
                    .await
            }
            .await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::OperationCancelled,
                ErrorKind::Cancelled,
                "loader resolution was cancelled",
            )
        });
        LoaderResolveOperation { operation, future }
    }

    pub(crate) async fn resolve_for_install(
        context: Arc<ServiceContext>,
        kind: LoaderKind,
        minecraft: &MinecraftVersionId,
        selector: LoaderVersionSelector,
        controller: &graphene_core::OperationController,
    ) -> Result<ResolvedLoader> {
        let provider = context.loader_registry.provider(kind)?;
        let exact = match selector {
            LoaderVersionSelector::Exact(version) => version,
            LoaderVersionSelector::LatestStable => provider
                .list_versions(minecraft, controller)
                .await?
                .into_iter()
                .find(|candidate| candidate.stable != Some(false))
                .map(|candidate| candidate.version)
                .ok_or_else(|| selector_error(kind, "no stable loader release is available"))?,
            LoaderVersionSelector::Recommended => provider
                .list_versions(minecraft, controller)
                .await?
                .into_iter()
                .find(|candidate| candidate.recommended == Some(true))
                .map(|candidate| candidate.version)
                .ok_or_else(|| {
                    selector_error(kind, "no recommended loader release is available")
                })?,
            _ => {
                return Err(selector_error(
                    kind,
                    "loader selector variant is unsupported",
                ));
            }
        };
        provider.resolve_exact(minecraft, &exact, controller).await
    }

    #[must_use]
    pub fn support(resolved: &ResolvedLoader) -> &LoaderSupport {
        &resolved.support
    }
}

pub struct LoaderVersionsOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = Result<Vec<LoaderVersionSummary>>> + Send + 'static>>,
}

impl LoaderVersionsOperation {
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }

    pub async fn await_result(self) -> Result<Vec<LoaderVersionSummary>> {
        self.future.await
    }
}

pub struct LoaderResolveOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = Result<ResolvedLoader>> + Send + 'static>>,
}

impl LoaderResolveOperation {
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }

    pub async fn await_result(self) -> Result<ResolvedLoader> {
        self.future.await
    }
}

fn selector_error(kind: LoaderKind, message: &'static str) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::LoaderVersionNotFound,
        ErrorKind::Minecraft,
        message,
    )
    .with_context("loader", kind.to_string())
}
