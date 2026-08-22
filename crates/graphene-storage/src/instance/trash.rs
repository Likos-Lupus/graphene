use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use graphene_platform::ensure_directory;
use std::{fs, path::Path};

/// Manages instance quarantine and deletion transactions.
#[derive(Debug, Default, Clone, Copy)]
pub struct InstanceTrash;

impl InstanceTrash {
    /// Atomically moves `instance_dir` into `trash_dir` on the same filesystem.
    ///
    /// A successful quarantine rename represents the logical commit of an instance deletion.
    pub fn quarantine(instance_dir: &Path, trash_dir: &Path) -> Result<()> {
        let metadata = fs::symlink_metadata(instance_dir).map_err(|source| {
            let code = if source.kind() == std::io::ErrorKind::NotFound {
                ErrorCode::InstanceNotFound
            } else {
                ErrorCode::FileOpenFailed
            };
            GrapheneError::new(code, ErrorKind::Instance, "instance directory not found")
                .with_context("path", instance_dir.display().to_string())
                .with_source(source)
        })?;

        if metadata.file_type().is_symlink() {
            return Err(GrapheneError::new(
                ErrorCode::InstanceInvalid,
                ErrorKind::Instance,
                "refusing to delete instance root that is a symbolic link",
            )
            .with_context("path", instance_dir.display().to_string()));
        }

        if let Some(parent) = trash_dir.parent() {
            ensure_directory(parent)?;
        }

        fs::rename(instance_dir, trash_dir).map_err(|source| {
            GrapheneError::new(
                ErrorCode::InstanceDeleteFailed,
                ErrorKind::Instance,
                "failed to move instance directory into quarantine trash",
            )
            .with_context("source", instance_dir.display().to_string())
            .with_context("destination", trash_dir.display().to_string())
            .with_source(source)
        })
    }

    /// Recursively removes a quarantined tree under `.trash/`.
    ///
    /// If physical cleanup fails (for instance due to an external file scanner), the error is
    /// reported while the instance remains safely quarantined.
    pub fn cleanup_quarantined_tree(trash_dir: &Path) -> Result<()> {
        if !trash_dir.exists() {
            return Ok(());
        }

        fs::remove_dir_all(trash_dir).map_err(|source| {
            GrapheneError::new(
                ErrorCode::InstanceDeleteFailed,
                ErrorKind::Instance,
                "quarantine trash cleanup encountered an error",
            )
            .with_context("path", trash_dir.display().to_string())
            .with_source(source)
        })
    }
}
