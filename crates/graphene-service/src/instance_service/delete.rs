use crate::{
    context::ServiceContext, instance_service::repository::InstanceRepository, operation_lifecycle,
};
use graphene_core::{
    ErrorCode, ErrorKind, GrapheneError, InstanceId, OperationController, OperationHandle, Result,
};
use graphene_instance::DeleteOptions;
use graphene_storage::InstanceTrash;
use std::{future::Future, pin::Pin, sync::Arc};

/// Prepared asynchronous instance delete operation handle.
pub struct InstanceDeleteOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = Result<()>> + Send>>,
}

impl InstanceDeleteOperation {
    /// Returns the public operation handle for progress observation and cancellation.
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }

    /// Awaits completion of the delete transaction.
    pub async fn await_result(self) -> Result<()> {
        self.future.await
    }
}

impl std::fmt::Debug for InstanceDeleteOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceDeleteOperation")
            .field("operation_id", &self.operation.id())
            .finish_non_exhaustive()
    }
}

pub(crate) fn start_delete_operation(
    context: Arc<ServiceContext>,
    instance_id: InstanceId,
    options: DeleteOptions,
) -> InstanceDeleteOperation {
    let controller = context.operations.create("instance-delete");
    let operation = controller.handle();
    let future = Box::pin(async move {
        operation_lifecycle::start(&controller)?;
        let result = execute_delete(context, instance_id, options, &controller).await;
        operation_lifecycle::finish(
            &controller,
            result,
            ErrorCode::OperationCancelled,
            ErrorKind::Cancelled,
            "instance delete operation was cancelled",
        )
    });

    InstanceDeleteOperation { operation, future }
}

async fn execute_delete(
    context: Arc<ServiceContext>,
    instance_id: InstanceId,
    _options: DeleteOptions,
    controller: &OperationController,
) -> Result<()> {
    controller.set_stage("acquire-lease")?;
    let repo = InstanceRepository::new(context.storage.path());
    let _lease = repo.acquire_exclusive_lease(instance_id)?;

    controller.set_stage("verify-instance")?;
    let instance_root = repo.paths().instance_root(instance_id);
    if !instance_root.exists() {
        return Err(GrapheneError::new(
            ErrorCode::InstanceNotFound,
            ErrorKind::Instance,
            "instance root directory does not exist",
        )
        .with_context("instance_id", instance_id.to_string()));
    }

    controller.set_stage("quarantine")?;
    let trash_dir = repo
        .paths()
        .trash_dir(instance_id, controller.handle().id());

    // Atomically move active instance directory into quarantine trash (logical delete commit)
    InstanceTrash::quarantine(&instance_root, &trash_dir)?;

    controller.set_stage("commit")?;
    // Seal cancellation: instance is already quarantined, so delete cannot be undone
    let _ = controller.seal_cancellation();

    controller.set_stage("cleanup")?;
    // Best-effort physical removal of the quarantined tree
    let _ = InstanceTrash::cleanup_quarantined_tree(&trash_dir);

    Ok(())
}
