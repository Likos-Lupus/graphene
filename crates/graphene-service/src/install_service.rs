use crate::{
    adapters::{ServiceArtifactAcquirer, ServiceMetadataAcquirer},
    component_install::plan_component_install,
    context::ServiceContext,
    install_tool_runner::ServiceInstallToolRunner,
    operation_lifecycle,
};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, OperationHandle, Result};
use graphene_install::{ComponentInstallRequest, InstallExecutor, InstallPlan, InstallRequest};
use graphene_instance::CommittedInstance;
use graphene_minecraft::{MinecraftArch, MinecraftOs, RuleContext};
use graphene_platform::{Architecture, OperatingSystem};
use graphene_providers::MojangProvider;
use std::{collections::BTreeMap, fs, future::Future, pin::Pin, sync::Arc};

#[derive(Clone)]
pub struct InstallService {
    context: Arc<ServiceContext>,
}

impl std::fmt::Debug for InstallService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallService").finish_non_exhaustive()
    }
}

impl InstallService {
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        Self { context }
    }

    #[must_use]
    pub fn plan(&self, request: InstallRequest) -> InstallPlanOperation {
        let controller = self.context.operations.create("install-plan");
        let operation = controller.handle();
        let context = Arc::clone(&self.context);
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = async {
                controller.set_stage("validate-request")?;
                request.instance.validate().map_err(|source| {
                    GrapheneError::new(
                        ErrorCode::InstallRequestInvalid,
                        ErrorKind::Install,
                        "install request contains invalid instance data",
                    )
                    .with_source(source)
                })?;
                let target = context
                    .storage
                    .path()
                    .join("instances")
                    .join(request.instance.id.to_string());
                match fs::symlink_metadata(&target) {
                    Ok(_) => {
                        return Err(GrapheneError::new(
                            ErrorCode::InstallTargetExists,
                            ErrorKind::Install,
                            "instance target already exists",
                        ));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(source) => {
                        return Err(GrapheneError::new(
                            ErrorCode::InstallStageFailed,
                            ErrorKind::Install,
                            "failed to inspect instance target",
                        )
                        .with_source(source));
                    }
                }

                controller.set_stage("resolve-minecraft")?;
                let provider =
                    MojangProvider::new(context.network.clone(), context.provider_config.clone())?;
                let metadata = ServiceMetadataAcquirer::new(Arc::clone(&context));
                let rules =
                    current_rule_context(context.platform.os, context.platform.architecture);
                let bundle = provider
                    .resolve(&request.minecraft_version, &rules, &metadata, &controller)
                    .await?;

                controller.set_stage("build-plan")?;
                InstallPlan::build(request, bundle.minecraft, bundle.metadata_artifacts)
            }
            .await;

            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::InstallCancelled,
                ErrorKind::Cancelled,
                "installation planning was cancelled",
            )
        });

        InstallPlanOperation { operation, future }
    }

    #[must_use]
    pub fn plan_components(&self, request: ComponentInstallRequest) -> InstallPlanOperation {
        let controller = self.context.operations.create("component-install-plan");
        let operation = controller.handle();
        let context = Arc::clone(&self.context);
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = plan_component_install(context, request, &controller).await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::InstallCancelled,
                ErrorKind::Cancelled,
                "component installation planning was cancelled",
            )
        });
        InstallPlanOperation { operation, future }
    }

    #[must_use]
    pub fn execute(&self, plan: InstallPlan) -> InstallExecutionOperation {
        let controller = self.context.operations.create("install-execute");
        let operation = controller.handle();
        let context = Arc::clone(&self.context);
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let acquirer = Arc::new(ServiceArtifactAcquirer::new(Arc::clone(&context)));
            let executor = InstallExecutor::new(context.storage.clone(), acquirer)
                .with_tool_runner(Arc::new(ServiceInstallToolRunner::new(Arc::clone(
                    &context,
                ))));
            let result = executor.execute(plan, &controller).await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::InstallCancelled,
                ErrorKind::Cancelled,
                "installation was cancelled",
            )
        });

        InstallExecutionOperation { operation, future }
    }
}

pub struct InstallPlanOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = Result<InstallPlan>> + Send + 'static>>,
}

impl InstallPlanOperation {
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }
    pub async fn await_result(self) -> Result<InstallPlan> {
        self.future.await
    }
}

pub struct InstallExecutionOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = Result<CommittedInstance>> + Send + 'static>>,
}

impl InstallExecutionOperation {
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }
    pub async fn await_result(self) -> Result<CommittedInstance> {
        self.future.await
    }
}

pub(crate) fn current_rule_context(os: OperatingSystem, architecture: Architecture) -> RuleContext {
    RuleContext {
        os: match os {
            OperatingSystem::Windows => MinecraftOs::Windows,
            OperatingSystem::Linux => MinecraftOs::Linux,
            OperatingSystem::MacOS => MinecraftOs::Osx,
            OperatingSystem::Other => MinecraftOs::Other("other".into()),
        },
        arch: match architecture {
            Architecture::X86 => MinecraftArch::X86,
            Architecture::X86_64 => MinecraftArch::X86_64,
            Architecture::AArch64 => MinecraftArch::AArch64,
            Architecture::Other => MinecraftArch::Other("other".into()),
        },
        os_version: None,
        features: BTreeMap::new(),
    }
}
