pub(crate) mod clone;
pub(crate) mod delete;
pub(crate) mod inventory;
pub(crate) mod repair;
pub(crate) mod repository;
#[cfg(test)]
mod tests;
pub(crate) mod verification;

pub use clone::InstanceCloneOperation;
pub use delete::InstanceDeleteOperation;
pub use repair::InstanceRepairOperation;
pub use repository::InstanceRepository;
pub use verification::InstanceVerifyOperation;

use crate::context::ServiceContext;
use graphene_core::{InstanceId, Result};
use graphene_instance::{
    CloneRequest, CommittedInstance, DeleteOptions, EffectiveInstanceConfig, GlobalInstanceConfig,
    InstanceConfigPatch, InstanceDescriptor, InstanceInventoryEntry, RepairOptions, RepairPlan,
    VerificationMode, apply_patch_to_global, apply_patch_to_instance,
};
use std::sync::Arc;

/// Service for inspecting, configuring, verifying, and managing instance lifecycles.
#[derive(Clone)]
pub struct InstanceService {
    #[allow(dead_code)]
    context: Arc<ServiceContext>,
    repository: InstanceRepository,
}

impl InstanceService {
    /// Creates a new instance service backed by the shared service context.
    #[must_use]
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        let repository = InstanceRepository::new(context.storage.path());
        Self {
            context,
            repository,
        }
    }

    /// Returns the shared service context.
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn context(&self) -> &Arc<ServiceContext> {
        &self.context
    }

    /// Returns the underlying committed instance repository.
    #[must_use]
    pub fn repository(&self) -> &InstanceRepository {
        &self.repository
    }

    /// Lists all instances found in storage in deterministic identifier order.
    pub async fn list(&self) -> Result<Vec<InstanceInventoryEntry>> {
        inventory::scan_inventory(&self.repository)
    }

    /// Loads the committed metadata record for `instance_id`.
    pub async fn get(&self, instance_id: InstanceId) -> Result<CommittedInstance> {
        self.repository.load_committed(instance_id)
    }

    /// Renames the instance's user-visible display name under an exclusive lease.
    ///
    /// Modifies only `instance.json` metadata without touching physical directories or runtime state.
    pub async fn rename(
        &self,
        instance_id: InstanceId,
        display_name: impl Into<String>,
    ) -> Result<InstanceDescriptor> {
        let display_name = display_name.into();
        let _lease = self.repository.acquire_exclusive_lease(instance_id)?;
        let mut descriptor = self.repository.load_descriptor(instance_id)?;
        descriptor.display_name = display_name;
        descriptor.validate()?;
        self.repository.write_descriptor(instance_id, &descriptor)?;
        Ok(descriptor)
    }

    /// Prepares an asynchronous instance clone operation.
    pub fn clone(&self, instance_id: InstanceId, request: CloneRequest) -> InstanceCloneOperation {
        clone::start_clone_operation(Arc::clone(&self.context), instance_id, request)
    }

    /// Prepares an asynchronous instance delete and quarantine transaction.
    pub fn delete(
        &self,
        instance_id: InstanceId,
        options: DeleteOptions,
    ) -> InstanceDeleteOperation {
        delete::start_delete_operation(Arc::clone(&self.context), instance_id, options)
    }

    /// Prepares an asynchronous structural or full cryptographic verification operation.
    pub fn verify(
        &self,
        instance_id: InstanceId,
        mode: VerificationMode,
    ) -> InstanceVerifyOperation {
        verification::start_verify_operation(Arc::clone(&self.context), instance_id, mode)
    }

    /// Derives a deterministic repair plan from durable desired state without mutating the filesystem.
    pub async fn plan_repair(
        &self,
        instance_id: InstanceId,
        options: RepairOptions,
    ) -> Result<RepairPlan> {
        repair::plan_repair(Arc::clone(&self.context), instance_id, options).await
    }

    /// Prepares an asynchronous repair execution operation.
    pub fn execute_repair(&self, plan: RepairPlan) -> InstanceRepairOperation {
        repair::start_repair_operation(Arc::clone(&self.context), plan)
    }

    /// Loads the current global instance defaults.
    pub async fn global_defaults(&self) -> Result<GlobalInstanceConfig> {
        self.repository.load_global_defaults()
    }

    /// Updates the global instance defaults by applying an explicit tri-state patch.
    pub async fn update_global_defaults(
        &self,
        patch: InstanceConfigPatch,
    ) -> Result<GlobalInstanceConfig> {
        let mut config = self.repository.load_global_defaults()?;
        apply_patch_to_global(&mut config, &patch);
        self.repository.write_global_defaults(&config)?;
        Ok(config)
    }

    /// Resolves the effective configuration for `instance_id` based on the hierarchy.
    pub async fn effective_config(
        &self,
        instance_id: InstanceId,
    ) -> Result<EffectiveInstanceConfig> {
        self.repository.effective_config(instance_id)
    }

    /// Updates per-instance configuration overrides under an exclusive lease.
    pub async fn update_config(
        &self,
        instance_id: InstanceId,
        patch: InstanceConfigPatch,
    ) -> Result<EffectiveInstanceConfig> {
        let _lease = self.repository.acquire_exclusive_lease(instance_id)?;
        let mut config = self
            .repository
            .load_instance_config(instance_id)?
            .unwrap_or_default();
        apply_patch_to_instance(&mut config, &patch);
        self.repository
            .write_instance_config(instance_id, &config)?;
        self.repository.effective_config(instance_id)
    }
}
