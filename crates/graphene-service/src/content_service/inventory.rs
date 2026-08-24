use crate::{context::ServiceContext, instance_service::repository::InstanceRepository};
use graphene_content::{LocalContentInventory, scan_local_inventory};
use graphene_core::{InstanceId, OperationController, Result};
use std::sync::Arc;

/// Scans the local `.minecraft/mods` inventory for an instance strictly offline.
pub async fn scan_instance_inventory(
    context: &Arc<ServiceContext>,
    instance_id: InstanceId,
    compute_hashes: bool,
    operation: &OperationController,
) -> Result<LocalContentInventory> {
    let repo = InstanceRepository::new(context.storage.path());
    let _lease = repo.acquire_shared_lease(instance_id)?;
    let mods_dir = repo.paths().minecraft_dir(instance_id).join("mods");
    scan_local_inventory(&mods_dir, compute_hashes, &operation.cancellation_token())
}
