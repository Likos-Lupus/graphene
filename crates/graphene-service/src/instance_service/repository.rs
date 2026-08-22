use graphene_core::{ErrorCode, ErrorKind, GrapheneError, InstanceId, Result};
use graphene_instance::{
    CommittedInstance, EffectiveInstanceConfig, GlobalInstanceConfig, InstallReceipt,
    InstanceConfig, InstanceDescriptor, InstanceStatus, resolve_effective_config,
};
use graphene_storage::{
    InstanceExclusiveLease, InstanceLeaseStore, InstancePaths, InstanceSharedLease,
    MAX_CONFIG_BYTES, MAX_DESCRIPTOR_BYTES, MAX_LOCKFILE_BYTES, MAX_RECEIPT_BYTES,
    read_document_bounded, read_json_bounded, write_json_atomic,
};
use std::path::Path;

/// Common committed-state repository for loading, validating, and persisting instance metadata.
#[derive(Debug, Clone)]
pub struct InstanceRepository {
    paths: InstancePaths,
    leases: InstanceLeaseStore,
}

impl InstanceRepository {
    /// Creates a new instance repository for the given data root.
    #[must_use]
    pub fn new(data_root: &Path) -> Self {
        let paths = InstancePaths::new(data_root);
        let leases = InstanceLeaseStore::new(paths.clone());
        Self { paths, leases }
    }

    /// Returns the physical instance paths calculator.
    #[must_use]
    pub fn paths(&self) -> &InstancePaths {
        &self.paths
    }

    /// Returns the lease manager.
    #[must_use]
    pub fn leases(&self) -> &InstanceLeaseStore {
        &self.leases
    }

    /// Acquires a shared lease on `instance_id`.
    pub fn acquire_shared_lease(&self, instance_id: InstanceId) -> Result<InstanceSharedLease> {
        self.leases.acquire_shared(instance_id)
    }

    /// Acquires an exclusive lease on `instance_id`.
    pub fn acquire_exclusive_lease(
        &self,
        instance_id: InstanceId,
    ) -> Result<InstanceExclusiveLease> {
        self.leases.acquire_exclusive(instance_id)
    }

    /// Loads and validates the committed `InstanceDescriptor` (`instance.json`).
    pub fn load_descriptor(&self, instance_id: InstanceId) -> Result<InstanceDescriptor> {
        let path = self.paths.instance_descriptor_path(instance_id);
        let descriptor: InstanceDescriptor = read_json_bounded(&path, MAX_DESCRIPTOR_BYTES)?;
        descriptor.validate()?;
        if descriptor.instance_id != instance_id {
            return Err(GrapheneError::new(
                ErrorCode::InstanceInvalid,
                ErrorKind::Instance,
                "instance descriptor contains mismatched instance id",
            )
            .with_context("expected", instance_id.to_string())
            .with_context("actual", descriptor.instance_id.to_string()));
        }
        Ok(descriptor)
    }

    /// Loads and validates the committed `InstallReceipt` (`.graphene/install.json`).
    pub fn load_receipt(&self, instance_id: InstanceId) -> Result<InstallReceipt> {
        let path = self.paths.install_receipt_path(instance_id);
        let receipt: InstallReceipt = read_json_bounded(&path, MAX_RECEIPT_BYTES)?;
        receipt.validate()?;
        if receipt.instance_id != instance_id {
            return Err(GrapheneError::new(
                ErrorCode::InstanceInvalid,
                ErrorKind::Instance,
                "install receipt contains mismatched instance id",
            )
            .with_context("expected", instance_id.to_string())
            .with_context("actual", receipt.instance_id.to_string()));
        }
        Ok(receipt)
    }

    /// Loads the complete committed instance record.
    pub fn load_committed(&self, instance_id: InstanceId) -> Result<CommittedInstance> {
        let descriptor = self.load_descriptor(instance_id)?;
        let receipt = self.load_receipt(instance_id)?;

        let lockfile_path = self.paths.lockfile_path(instance_id);
        let status = if lockfile_path.exists() {
            let _ = read_document_bounded(&lockfile_path, MAX_LOCKFILE_BYTES)?;
            InstanceStatus::Ready
        } else {
            InstanceStatus::Legacy
        };

        Ok(CommittedInstance::new(descriptor, receipt, status))
    }

    /// Atomically writes an updated `InstanceDescriptor` to `instance.json`.
    pub fn write_descriptor(
        &self,
        instance_id: InstanceId,
        descriptor: &InstanceDescriptor,
    ) -> Result<()> {
        descriptor.validate()?;
        if descriptor.instance_id != instance_id {
            return Err(GrapheneError::new(
                ErrorCode::InstanceInvalid,
                ErrorKind::Instance,
                "cannot write descriptor with mismatched instance id",
            ));
        }
        let path = self.paths.instance_descriptor_path(instance_id);
        write_json_atomic(&path, descriptor)
    }

    /// Loads global instance defaults from `config/instance-defaults.json`.
    pub fn load_global_defaults(&self) -> Result<GlobalInstanceConfig> {
        let path = self.paths.global_defaults_path();
        if !path.exists() {
            return Ok(GlobalInstanceConfig::default());
        }
        let config: GlobalInstanceConfig = read_json_bounded(&path, MAX_CONFIG_BYTES)?;
        config.validate()?;
        Ok(config)
    }

    /// Atomically writes global instance defaults to `config/instance-defaults.json`.
    pub fn write_global_defaults(&self, config: &GlobalInstanceConfig) -> Result<()> {
        config.validate()?;
        let path = self.paths.global_defaults_path();
        write_json_atomic(&path, config)
    }

    /// Loads per-instance configuration overrides from `.graphene/config.json`.
    pub fn load_instance_config(&self, instance_id: InstanceId) -> Result<Option<InstanceConfig>> {
        let path = self.paths.config_path(instance_id);
        if !path.exists() {
            return Ok(None);
        }
        let config: InstanceConfig = read_json_bounded(&path, MAX_CONFIG_BYTES)?;
        config.validate()?;
        Ok(Some(config))
    }

    /// Atomically writes per-instance configuration overrides to `.graphene/config.json`.
    pub fn write_instance_config(
        &self,
        instance_id: InstanceId,
        config: &InstanceConfig,
    ) -> Result<()> {
        config.validate()?;
        let path = self.paths.config_path(instance_id);
        write_json_atomic(&path, config)
    }

    /// Resolves the effective configuration for an instance based on the hierarchy.
    pub fn effective_config(&self, instance_id: InstanceId) -> Result<EffectiveInstanceConfig> {
        let global = self.load_global_defaults()?;
        let instance = self.load_instance_config(instance_id)?;
        Ok(resolve_effective_config(Some(&global), instance.as_ref()))
    }
}
