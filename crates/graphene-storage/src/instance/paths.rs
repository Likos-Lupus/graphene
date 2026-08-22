use graphene_core::{InstanceId, OperationId};
use std::path::{Path, PathBuf};

/// Physical path calculations for an instance within a data root.
#[derive(Debug, Clone)]
pub struct InstancePaths {
    data_root: PathBuf,
}

impl InstancePaths {
    /// Creates a new instance path calculator for the given data root.
    #[must_use]
    pub fn new(data_root: impl Into<PathBuf>) -> Self {
        Self {
            data_root: data_root.into(),
        }
    }

    /// Returns the underlying data root path.
    #[must_use]
    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    /// Returns the path to `instances/`.
    #[must_use]
    pub fn instances_dir(&self) -> PathBuf {
        self.data_root.join("instances")
    }

    /// Returns the root path for a specific instance (`instances/<instance-id>`).
    #[must_use]
    pub fn instance_root(&self, id: InstanceId) -> PathBuf {
        self.instances_dir().join(id.to_string())
    }

    /// Returns the directory holding persistent lock carrier files (`instances/.locks`).
    #[must_use]
    pub fn locks_dir(&self) -> PathBuf {
        self.instances_dir().join(".locks")
    }

    /// Returns the path to the persistent lock carrier file for an instance.
    #[must_use]
    pub fn lock_carrier_path(&self, id: InstanceId) -> PathBuf {
        self.locks_dir().join(format!("{id}.lock"))
    }

    /// Returns the staging root directory (`instances/.staging`).
    #[must_use]
    pub fn staging_root(&self) -> PathBuf {
        self.instances_dir().join(".staging")
    }

    /// Returns the staging directory for a specific operation.
    #[must_use]
    pub fn staging_dir(&self, operation_id: OperationId) -> PathBuf {
        self.staging_root().join(operation_id.to_string())
    }

    /// Returns the quarantine root directory (`instances/.trash`).
    #[must_use]
    pub fn trash_root(&self) -> PathBuf {
        self.instances_dir().join(".trash")
    }

    /// Returns the quarantine destination for an instance deletion operation.
    #[must_use]
    pub fn trash_dir(&self, id: InstanceId, operation_id: OperationId) -> PathBuf {
        self.trash_root().join(format!("{id}-{operation_id}"))
    }

    /// Returns the path to `instance.json` for an instance.
    #[must_use]
    pub fn instance_descriptor_path(&self, id: InstanceId) -> PathBuf {
        self.instance_root(id).join("instance.json")
    }

    /// Returns the path to `.graphene/install.json` for an instance.
    #[must_use]
    pub fn install_receipt_path(&self, id: InstanceId) -> PathBuf {
        self.instance_root(id)
            .join(".graphene")
            .join("install.json")
    }

    /// Returns the path to `.graphene/lock.json` for an instance.
    #[must_use]
    pub fn lockfile_path(&self, id: InstanceId) -> PathBuf {
        self.instance_root(id).join(".graphene").join("lock.json")
    }

    /// Returns the path to `.graphene/config.json` for an instance.
    #[must_use]
    pub fn config_path(&self, id: InstanceId) -> PathBuf {
        self.instance_root(id).join(".graphene").join("config.json")
    }

    /// Returns the path to `config/instance-defaults.json`.
    #[must_use]
    pub fn global_defaults_path(&self) -> PathBuf {
        self.data_root.join("config").join("instance-defaults.json")
    }

    /// Returns the path to `.minecraft` game working directory for an instance.
    #[must_use]
    pub fn minecraft_dir(&self, id: InstanceId) -> PathBuf {
        self.instance_root(id).join(".minecraft")
    }
}
