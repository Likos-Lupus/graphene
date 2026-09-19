use crate::{context::ServiceContext, operation_lifecycle};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, InstanceId, OperationHandle, Result};
use graphene_java::{
    JavaArchitecture, JavaRequirement, JavaRuntime, ManagedJavaRuntime, select_java,
    select_managed_runtime,
};
use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};

mod install;
mod inventory;
mod support;

use inventory::{load_inventory, validate_and_probe_committed};
use support::{java_error, spawn_blocking_java};

#[derive(Clone)]
pub struct JavaService {
    context: Arc<ServiceContext>,
}

impl std::fmt::Debug for JavaService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JavaService").finish_non_exhaustive()
    }
}

impl JavaService {
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        Self { context }
    }

    /// Side-effect-free selection: explicit override, compatible local Java, then committed managed Java.
    pub async fn select_for_instance(
        &self,
        instance_id: InstanceId,
        explicit: Option<PathBuf>,
    ) -> Result<JavaRuntime> {
        let requirement = self.requirement_for_instance(instance_id).await?;
        if explicit.is_some() {
            return select_java(&requirement, explicit).await;
        }

        match select_java(&requirement, None).await {
            Ok(runtime) => Ok(runtime),
            Err(local_error)
                if matches!(
                    local_error.code,
                    ErrorCode::JavaNotFound | ErrorCode::JavaIncompatible
                ) =>
            {
                match self.select_committed_managed(&requirement).await {
                    Ok(runtime) => Ok(runtime),
                    Err(managed_error) if managed_error.code == ErrorCode::JavaNotFound => {
                        Err(local_error)
                    }
                    Err(managed_error) => Err(managed_error),
                }
            }
            Err(error) => Err(error),
        }
    }

    /// Side-effect-free Java selection for an already normalized requirement.
    pub(crate) async fn select_for_requirement(
        &self,
        requirement: &JavaRequirement,
    ) -> Result<JavaRuntime> {
        match select_java(requirement, None).await {
            Ok(runtime) => Ok(runtime),
            Err(local_error)
                if matches!(
                    local_error.code,
                    ErrorCode::JavaNotFound | ErrorCode::JavaIncompatible
                ) =>
            {
                match self.select_committed_managed(requirement).await {
                    Ok(runtime) => Ok(runtime),
                    Err(managed_error) if managed_error.code == ErrorCode::JavaNotFound => {
                        Err(local_error)
                    }
                    Err(managed_error) => Err(managed_error),
                }
            }
            Err(error) => Err(error),
        }
    }

    /// Explicitly permits managed installation when no existing runtime satisfies the instance.
    #[must_use]
    pub fn ensure_for_instance(
        &self,
        instance_id: InstanceId,
        explicit: Option<PathBuf>,
    ) -> ManagedJavaOperation {
        let controller = self.context.operations.create("ensure-java");
        let operation = controller.handle();
        let service = self.clone();
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = async {
                controller.set_stage("inspect_local_runtimes")?;
                let requirement = service.requirement_for_instance(instance_id).await?;
                if explicit.is_some() {
                    return select_java(&requirement, explicit).await;
                }

                match select_java(&requirement, None).await {
                    Ok(runtime) => return Ok(runtime),
                    Err(error)
                        if matches!(
                            error.code,
                            ErrorCode::JavaNotFound | ErrorCode::JavaIncompatible
                        ) => {}
                    Err(error) => return Err(error),
                }

                controller.set_stage("inspect_managed_runtimes")?;
                match service.select_committed_managed(&requirement).await {
                    Ok(runtime) => return Ok(runtime),
                    Err(error) if error.code == ErrorCode::JavaNotFound => {}
                    Err(error) => return Err(error),
                }

                service
                    .install_managed_inner(requirement, &controller)
                    .await
            }
            .await;

            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::OperationCancelled,
                ErrorKind::Java,
                "managed Java ensure operation was cancelled",
            )
        });

        ManagedJavaOperation { operation, future }
    }

    /// Explicit managed-runtime installation operation for an already normalized requirement.
    #[must_use]
    pub fn install_managed(&self, requirement: JavaRequirement) -> ManagedJavaOperation {
        let controller = self.context.operations.create("install-managed-java");
        let operation = controller.handle();
        let service = self.clone();
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = service
                .install_managed_inner(requirement, &controller)
                .await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::OperationCancelled,
                ErrorKind::Java,
                "managed Java installation was cancelled",
            )
        });
        ManagedJavaOperation { operation, future }
    }

    pub async fn managed_runtimes(&self) -> Result<Vec<ManagedJavaRuntime>> {
        let root = self.context.storage.clone();
        spawn_blocking_java(move || load_inventory(&root)).await
    }

    pub(crate) async fn requirement_for_instance(
        &self,
        instance_id: InstanceId,
    ) -> Result<JavaRequirement> {
        let repo = crate::instance_service::InstanceRepository::new(self.context.storage.path());
        let receipt = repo.load_receipt(instance_id).map_err(|source| {
            GrapheneError::new(
                ErrorCode::LaunchInstanceInvalid,
                ErrorKind::Launch,
                "committed install receipt is unavailable for Java selection",
            )
            .with_source(source)
        })?;

        Ok(JavaRequirement {
            major_version: receipt.java_requirement.major_version,
            component_hint: receipt.java_requirement.component_hint.clone(),
        })
    }

    async fn select_committed_managed(&self, requirement: &JavaRequirement) -> Result<JavaRuntime> {
        let runtimes = self.managed_runtimes().await?;
        let descriptor =
            select_managed_runtime(requirement, JavaArchitecture::current(), &runtimes)
                .ok_or_else(|| {
                    java_error(
                        ErrorCode::JavaNotFound,
                        "no compatible committed managed Java runtime was found",
                    )
                })?;
        let runtime_dir = self
            .context
            .storage
            .path()
            .join("shared/runtimes")
            .join(descriptor.id.to_string());
        validate_and_probe_committed(descriptor, runtime_dir, requirement).await
    }
}

pub struct ManagedJavaOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = Result<JavaRuntime>> + Send>>,
}

impl ManagedJavaOperation {
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }
    pub async fn await_result(self) -> Result<JavaRuntime> {
        self.future.await
    }
}

impl std::fmt::Debug for ManagedJavaOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedJavaOperation")
            .field("operation_id", &self.operation.id())
            .finish_non_exhaustive()
    }
}
