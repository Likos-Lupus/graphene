use crate::{
    context::ServiceContext, instance_service::repository::InstanceRepository, operation_lifecycle,
};
use graphene_core::{
    ErrorCode, ErrorKind, GrapheneError, InstanceId, OperationController, OperationHandle, Result,
};
use graphene_instance::{
    CloneRequest, CommittedInstance, InstallReceipt, InstanceDescriptor, InstanceLockfile,
};
use graphene_storage::{
    InstanceStagingTree, MAX_CLONE_TOTAL_BYTES, MAX_CONFIG_BYTES, MAX_DESCRIPTOR_BYTES,
    MAX_LOCKFILE_BYTES, MAX_RECEIPT_BYTES, clone_instance_tree, read_document_bounded,
    write_json_atomic,
};
use std::{future::Future, pin::Pin, sync::Arc};

/// Prepared asynchronous instance clone operation handle.
pub struct InstanceCloneOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = Result<CommittedInstance>> + Send>>,
}

impl InstanceCloneOperation {
    /// Returns the public operation handle for progress observation and cancellation.
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }

    /// Awaits completion of the clone transaction.
    pub async fn await_result(self) -> Result<CommittedInstance> {
        self.future.await
    }
}

impl std::fmt::Debug for InstanceCloneOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceCloneOperation")
            .field("operation_id", &self.operation.id())
            .finish_non_exhaustive()
    }
}

pub(crate) fn start_clone_operation(
    context: Arc<ServiceContext>,
    source_id: InstanceId,
    request: CloneRequest,
) -> InstanceCloneOperation {
    let controller = context.operations.create("instance-clone");
    let operation = controller.handle();
    let future = Box::pin(async move {
        operation_lifecycle::start(&controller)?;
        let result = execute_clone(context, source_id, request, &controller).await;
        operation_lifecycle::finish(
            &controller,
            result,
            ErrorCode::OperationCancelled,
            ErrorKind::Cancelled,
            "instance clone operation was cancelled",
        )
    });

    InstanceCloneOperation { operation, future }
}

async fn execute_clone(
    context: Arc<ServiceContext>,
    source_id: InstanceId,
    request: CloneRequest,
    controller: &OperationController,
) -> Result<CommittedInstance> {
    controller.set_stage("validate-request")?;
    request.validate(source_id)?;

    let dest_id = request.destination_id;
    let repo = InstanceRepository::new(context.storage.path());

    // Acquire exclusive leases in sorted ID order
    let (_source_lease, _dest_lease) = repo.leases().acquire_exclusive_pair(source_id, dest_id)?;

    controller.set_stage("load-source")?;
    let source_committed = repo.load_committed(source_id)?;

    let source_root = repo.paths().instance_root(source_id);
    let dest_root = repo.paths().instance_root(dest_id);

    if dest_root.exists() {
        return Err(GrapheneError::new(
            ErrorCode::InstallTargetExists,
            ErrorKind::Instance,
            "clone destination instance target already exists on disk",
        )
        .with_context("destination_id", dest_id.to_string()));
    }

    controller.set_stage("stage-tree")?;
    let staging =
        InstanceStagingTree::create(&repo.paths().staging_root(), controller.handle().id())?;

    // Copy source directory tree into staging
    clone_instance_tree(&source_root, staging.path(), MAX_CLONE_TOTAL_BYTES)?;

    controller.set_stage("rewrite-metadata")?;
    // 1. Rewrite instance.json
    let mut descriptor = source_committed.descriptor;
    descriptor.instance_id = dest_id;
    descriptor.display_name = request.destination_display_name;
    descriptor.validate()?;
    write_json_atomic(&staging.path().join("instance.json"), &descriptor)?;

    // 2. Rewrite .graphene/install.json
    let mut receipt = source_committed.receipt;
    receipt.instance_id = dest_id;
    receipt.validate()?;
    write_json_atomic(&staging.path().join(".graphene/install.json"), &receipt)?;

    // 3. Rewrite .graphene/lock.json if present
    let staged_lockfile = staging.path().join(".graphene/lock.json");
    if staged_lockfile.exists() {
        let bytes = read_document_bounded(&staged_lockfile, MAX_LOCKFILE_BYTES)?;
        if let Ok(mut lockfile) = serde_json::from_slice::<InstanceLockfile>(&bytes) {
            lockfile.instance_id = dest_id;
            lockfile.validate()?;
            write_json_atomic(&staged_lockfile, &lockfile)?;
        }
    }

    // 4. Rewrite .graphene/config.json if present
    let staged_config = staging.path().join(".graphene/config.json");
    if staged_config.exists() {
        let bytes = read_document_bounded(&staged_config, MAX_CONFIG_BYTES)?;
        if let Ok(config) = serde_json::from_slice::<graphene_instance::InstanceConfig>(&bytes) {
            config.validate()?;
            write_json_atomic(&staged_config, &config)?;
        }
    }

    controller.set_stage("validate-staging")?;
    let staged_desc: InstanceDescriptor = graphene_storage::read_json_bounded(
        &staging.path().join("instance.json"),
        MAX_DESCRIPTOR_BYTES,
    )?;
    staged_desc.validate()?;

    let staged_rec: InstallReceipt = graphene_storage::read_json_bounded(
        &staging.path().join(".graphene/install.json"),
        MAX_RECEIPT_BYTES,
    )?;
    staged_rec.validate()?;

    controller.set_stage("commit")?;
    if !controller.seal_cancellation() {
        return Err(GrapheneError::new(
            ErrorCode::OperationCancelled,
            ErrorKind::Cancelled,
            "clone operation was cancelled prior to commit",
        ));
    }

    staging.publish_create_only(&dest_root)?;

    repo.load_committed(dest_id)
}
