use crate::{context::ServiceContext, operation_lifecycle};
use graphene_core::{ErrorCode, ErrorKind, OperationHandle, Result};
use graphene_minecraft::VersionManifest;
use graphene_providers::MojangProvider;
use std::{future::Future, pin::Pin, sync::Arc};

#[derive(Clone)]
pub struct MinecraftService {
    context: Arc<ServiceContext>,
}

impl std::fmt::Debug for MinecraftService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MinecraftService").finish_non_exhaustive()
    }
}

impl MinecraftService {
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        Self { context }
    }

    #[must_use]
    pub fn versions(&self) -> MinecraftManifestOperation {
        let controller = self.context.operations.create("minecraft-versions");
        let operation = controller.handle();
        let context = Arc::clone(&self.context);
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = async {
                controller.set_stage("manifest")?;
                let provider =
                    MojangProvider::new(context.network.clone(), context.provider_config.clone())?;
                provider.manifest(&controller).await
            }
            .await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::DownloadCancelled,
                ErrorKind::Cancelled,
                "Minecraft metadata operation was cancelled",
            )
        });

        MinecraftManifestOperation { operation, future }
    }
}

pub struct MinecraftManifestOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = Result<VersionManifest>> + Send + 'static>>,
}

impl MinecraftManifestOperation {
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }
    pub async fn await_result(self) -> Result<VersionManifest> {
        self.future.await
    }
}
