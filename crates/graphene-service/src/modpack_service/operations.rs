use super::{
    export::{ModpackExportPlan, ModpackExportRequest, ModpackExportResult, plan_export},
    export_execute::execute_export,
    inspection::inspect_pack,
    planning::{ModpackImportPlan, ModpackImportRequest, plan_import},
    source::PackSource,
};
use crate::{
    adapters::ServiceArtifactAcquirer, context::ServiceContext,
    install_tool_runner::ServiceInstallToolRunner, operation_lifecycle,
};
use graphene_core::{ErrorCode, ErrorKind, OperationHandle, Result};
use graphene_install::InstallExecutor;
use std::{future::Future, pin::Pin, sync::Arc};

/// Host-facing modpack workflows: inspect -> plan import -> execute import.
///
/// All long-running operations use the shared operation lifecycle; results are Graphene-owned
/// values with URLs, cache paths, and provider DTOs excluded by construction.
#[derive(Clone)]
pub struct ModpackService {
    context: Arc<ServiceContext>,
}

impl std::fmt::Debug for ModpackService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModpackService").finish_non_exhaustive()
    }
}

impl ModpackService {
    #[allow(dead_code)]
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        Self { context }
    }

    /// Inspects a local file or HTTPS URL as an immutable pack-source snapshot without touching
    /// any instance state.
    #[must_use]
    pub fn inspect(&self, source: PackSource) -> ModpackInspectionOperation {
        let controller = self.context.operations.create("modpack-inspect");
        let operation = controller.handle();
        let context = Arc::clone(&self.context);
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = inspect_pack(&context, &source, &controller).await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::DownloadCancelled,
                ErrorKind::Cancelled,
                "pack inspection was cancelled",
            )
        });
        ModpackInspectionOperation { operation, future }
    }

    /// Builds the deterministic composite create-only import plan.
    #[must_use]
    pub fn plan_import(&self, request: ModpackImportRequest) -> ModpackPlanOperation {
        let controller = self.context.operations.create("modpack-plan-import");
        let operation = controller.handle();
        let context = Arc::clone(&self.context);
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = plan_import(&context, &request, &controller).await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::DownloadCancelled,
                ErrorKind::Cancelled,
                "pack import planning was cancelled",
            )
        });
        ModpackPlanOperation { operation, future }
    }

    /// Executes a validated import plan through the single ordinary install transaction.
    #[must_use]
    pub fn execute_import(&self, plan: ModpackImportPlan) -> ModpackExecutionOperation {
        let controller = self.context.operations.create("modpack-execute-import");
        let operation = controller.handle();
        let context = Arc::clone(&self.context);
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let acquirer = Arc::new(ServiceArtifactAcquirer::new(Arc::clone(&context)));
            let executor = InstallExecutor::new(context.storage.clone(), acquirer)
                .with_tool_runner(Arc::new(ServiceInstallToolRunner::new(Arc::clone(
                    &context,
                ))));
            let result = executor.execute(plan.install_plan, &controller).await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::InstallCancelled,
                ErrorKind::Cancelled,
                "pack import execution was cancelled",
            )
        });
        ModpackExecutionOperation { operation, future }
    }

    /// Plans a conservative Graphene pack export from committed instance desired state.
    #[must_use]
    pub fn plan_export(&self, request: ModpackExportRequest) -> ModpackExportPlanOperation {
        let controller = self.context.operations.create("modpack-plan-export");
        let operation = controller.handle();
        let context = Arc::clone(&self.context);
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = plan_export(&context, &request, &controller).await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::DownloadCancelled,
                ErrorKind::Cancelled,
                "pack export planning was cancelled",
            )
        });
        ModpackExportPlanOperation { operation, future }
    }

    /// Executes a validated export plan and publishes the pack create-only.
    #[must_use]
    pub fn execute_export(&self, plan: ModpackExportPlan) -> ModpackExportExecutionOperation {
        let controller = self.context.operations.create("modpack-execute-export");
        let operation = controller.handle();
        let context = Arc::clone(&self.context);
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = execute_export(&context, &plan, &controller).await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::InstallCancelled,
                ErrorKind::Cancelled,
                "pack export execution was cancelled",
            )
        });
        ModpackExportExecutionOperation { operation, future }
    }
}

pub struct ModpackInspectionOperation {
    operation: OperationHandle,
    future:
        Pin<Box<dyn Future<Output = Result<graphene_modpack::PackInspection>> + Send + 'static>>,
}

impl ModpackInspectionOperation {
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }

    pub async fn await_result(self) -> Result<graphene_modpack::PackInspection> {
        self.future.await
    }
}

pub struct ModpackPlanOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = Result<ModpackImportPlan>> + Send + 'static>>,
}

impl ModpackPlanOperation {
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }

    pub async fn await_result(self) -> Result<ModpackImportPlan> {
        self.future.await
    }
}

pub struct ModpackExecutionOperation {
    operation: OperationHandle,
    future: Pin<
        Box<dyn Future<Output = Result<graphene_instance::CommittedInstance>> + Send + 'static>,
    >,
}

impl ModpackExecutionOperation {
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }

    pub async fn await_result(self) -> Result<graphene_instance::CommittedInstance> {
        self.future.await
    }
}

pub struct ModpackExportPlanOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = Result<ModpackExportPlan>> + Send + 'static>>,
}

impl ModpackExportPlanOperation {
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }

    pub async fn await_result(self) -> Result<ModpackExportPlan> {
        self.future.await
    }
}

pub struct ModpackExportExecutionOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = Result<ModpackExportResult>> + Send + 'static>>,
}

impl ModpackExportExecutionOperation {
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }

    pub async fn await_result(self) -> Result<ModpackExportResult> {
        self.future.await
    }
}
