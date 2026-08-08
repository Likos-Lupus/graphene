use crate::error::install_error;
use graphene_core::{ErrorCode, InstanceId, Result};
use graphene_platform::{ManagedRelativePath, ensure_managed_directory};
use std::{
    fs,
    io::ErrorKind as IoErrorKind,
    path::{Path, PathBuf},
};

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
    path: PathBuf,
    _file: fs::File,
}

impl InstanceInstallLock {
    pub(super) fn acquire(data_root: &Path, id: InstanceId) -> Result<Self> {
        let relative = ManagedRelativePath::new("instances/.install-locks")?;
        let root = ensure_managed_directory(data_root, &relative)?;
        let path = root.join(format!("{id}.lock"));
        let file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|source| {
                let code = if source.kind() == IoErrorKind::AlreadyExists {
                    ErrorCode::InstallTargetExists
                } else {
                    ErrorCode::InstallStageFailed
                };
                install_error(code, "failed to reserve create-only instance target")
                    .with_source(source)
            })?;

        Ok(Self { path, _file: file })
    }
}

impl Drop for InstanceInstallLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
