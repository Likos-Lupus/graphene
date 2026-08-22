use graphene_core::{ErrorCode, ErrorKind, GrapheneError, OperationId, Result};
use graphene_platform::{ensure_directory, publish_directory_create_only};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Managed staging tree for constructing an instance before atomic create-only publication.
#[derive(Debug)]
pub struct InstanceStagingTree {
    path: PathBuf,
    committed: bool,
}

impl InstanceStagingTree {
    /// Creates a new isolated staging tree under `staging_root` for `operation_id`.
    pub fn create(staging_root: &Path, operation_id: OperationId) -> Result<Self> {
        ensure_directory(staging_root)?;
        let path = staging_root.join(operation_id.to_string());

        if path.exists() {
            let _ = fs::remove_dir_all(&path);
        }

        fs::create_dir_all(&path).map_err(|source| {
            GrapheneError::new(
                ErrorCode::DirectoryCreateFailed,
                ErrorKind::Filesystem,
                "failed to create instance staging directory",
            )
            .with_context("path", path.display().to_string())
            .with_source(source)
        })?;

        Ok(Self {
            path,
            committed: false,
        })
    }

    /// Returns the staging directory path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes raw bytes to a relative path inside the staging tree.
    pub fn write_file(&self, relative_path: impl AsRef<Path>, bytes: &[u8]) -> Result<PathBuf> {
        let dest = self.path.join(relative_path.as_ref());
        if let Some(parent) = dest.parent() {
            ensure_directory(parent)?;
        }

        fs::write(&dest, bytes).map_err(|source| {
            GrapheneError::new(
                ErrorCode::FileWriteFailed,
                ErrorKind::Filesystem,
                "failed to write file into instance staging tree",
            )
            .with_context("path", dest.display().to_string())
            .with_source(source)
        })?;

        Ok(dest)
    }

    /// Commits the staging directory into `destination` using create-only atomic semantics.
    ///
    /// If `destination` already exists, publication fails without mutating `destination`.
    pub fn publish_create_only(mut self, destination: &Path) -> Result<()> {
        publish_directory_create_only(&self.path, destination).map_err(|source| {
            GrapheneError::new(
                ErrorCode::InstallCommitFailed,
                ErrorKind::Storage,
                "failed to publish staged instance directory",
            )
            .with_context("staging", self.path.display().to_string())
            .with_context("destination", destination.display().to_string())
            .with_source(source)
        })?;

        self.committed = true;
        Ok(())
    }
}

impl Drop for InstanceStagingTree {
    fn drop(&mut self) {
        if !self.committed && self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
