use crate::error::install_error;
use graphene_core::{ErrorCode, InstanceId, Result};
use graphene_storage::{InstanceExclusiveLease, InstanceLeaseStore, InstancePaths};
use std::{fs, io::ErrorKind as IoErrorKind, path::Path};

pub(super) fn reject_existing_target(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(install_error(
            ErrorCode::InstallTargetExists,
            "instance target already exists",
        )),
        Err(error) if error.kind() == IoErrorKind::NotFound => Ok(()),
        Err(source) => Err(install_error(
            ErrorCode::InstallStageFailed,
            "failed to inspect instance target",
        )
        .with_source(source)),
    }
}

pub(super) struct InstanceInstallLock {
    _lease: InstanceExclusiveLease,
}

impl InstanceInstallLock {
    pub(super) fn acquire(data_root: &Path, id: InstanceId) -> Result<Self> {
        let paths = InstancePaths::new(data_root);
        let store = InstanceLeaseStore::new(paths);
        let lease = store.acquire_exclusive(id).map_err(|err| {
            if err.code == ErrorCode::InstanceBusy {
                install_error(
                    ErrorCode::InstallTargetExists,
                    "failed to reserve instance target: instance is busy or lock is held",
                )
            } else {
                err
            }
        })?;

        Ok(Self { _lease: lease })
    }
}
