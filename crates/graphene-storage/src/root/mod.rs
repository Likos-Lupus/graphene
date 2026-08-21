use crate::{
    CacheAddress,
    atomic::write_new_atomic,
    layout::{DIRECTORIES, LAYOUT_VERSION, LayoutMarker, MARKER_FILE},
    temp::download_temp_path,
};
use graphene_core::{
    ArtifactId, ArtifactIntegrity, ErrorCode, ErrorKind, GrapheneError, OperationId, Result,
};
use graphene_platform::{
    ManagedRelativePath, ensure_directory, ensure_managed_directory, normalize_root,
    replace_file_safely,
};
use std::{
    fs,
    io::{ErrorKind as IoErrorKind, Write},
    path::{Path, PathBuf},
};

/// Explicit Graphene-managed data root. Each engine owns its own value.
#[derive(Debug, Clone)]
pub struct DataRoot {
    path: PathBuf,
}

impl DataRoot {
    /// Initializes an empty or existing root idempotently and validates the managed layout marker.
    pub fn initialize(path: impl AsRef<Path>) -> Result<Self> {
        let path = normalize_root(path)?;
        ensure_directory(&path)?;
        let canonical = fs::canonicalize(&path).map_err(|source| {
            GrapheneError::new(
                ErrorCode::DataRootInvalid,
                ErrorKind::Filesystem,
                "failed to canonicalize data root",
            )
            .with_source(source)
        })?;

        for relative in DIRECTORIES {
            let relative = ManagedRelativePath::new(relative)?;
            ensure_managed_directory(&canonical, &relative)?;
        }
        verify_write_capability(&canonical)?;

        let marker_path = canonical.join(MARKER_FILE);
        if fs::symlink_metadata(&marker_path)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(GrapheneError::new(
                ErrorCode::StorageLayoutInvalid,
                ErrorKind::Storage,
                "Graphene layout marker must not be a symbolic link",
            ));
        }

        if marker_path.exists() {
            let bytes = fs::read(&marker_path).map_err(|source| {
                GrapheneError::new(
                    ErrorCode::FileOpenFailed,
                    ErrorKind::Storage,
                    "failed to read Graphene layout marker",
                )
                .with_source(source)
            })?;
            let marker: LayoutMarker = serde_json::from_slice(&bytes).map_err(|source| {
                GrapheneError::new(
                    ErrorCode::StorageLayoutInvalid,
                    ErrorKind::Storage,
                    "Graphene layout marker is invalid",
                )
                .with_source(source)
            })?;

            if marker.layout_version != LAYOUT_VERSION {
                return Err(GrapheneError::new(
                    ErrorCode::StorageLayoutInvalid,
                    ErrorKind::Storage,
                    "unsupported Graphene data-root layout version",
                )
                .with_context("expected", LAYOUT_VERSION.to_string())
                .with_context("actual", marker.layout_version.to_string()));
            }
        } else {
            let marker = LayoutMarker {
                layout_version: LAYOUT_VERSION,
            };
            let bytes = serde_json::to_vec_pretty(&marker).map_err(|source| {
                GrapheneError::new(
                    ErrorCode::InternalInvariantViolation,
                    ErrorKind::Internal,
                    "failed to serialize layout marker",
                )
                .with_source(source)
            })?;

            write_new_atomic(&marker_path, &bytes)?;
        }

        Ok(Self { path: canonical })
    }

    /// Returns the canonical data-root path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the layout marker path.
    #[must_use]
    pub fn layout_marker_path(&self) -> PathBuf {
        self.path.join(MARKER_FILE)
    }

    /// Resolves the deterministic verified cache object path. SHA-256 is preferred when both are
    /// present.
    pub fn cache_address(&self, integrity: &ArtifactIntegrity) -> Result<CacheAddress> {
        CacheAddress::from_integrity(&self.path, integrity)
    }

    /// Returns a recognizable temporary path separate from committed cache objects after
    /// revalidating the managed temporary directory against symbolic-link substitution.
    pub fn download_temp_path(
        &self,
        artifact_id: ArtifactId,
        operation_id: OperationId,
    ) -> Result<PathBuf> {
        let relative = ManagedRelativePath::new("cache/downloads/temporary")?;
        let temporary_root = ensure_managed_directory(&self.path, &relative)?;

        Ok(download_temp_path(
            &temporary_root,
            artifact_id,
            operation_id,
        ))
    }

    /// Returns whether `path` is an ordinary committed file that resolves within this engine's
    /// managed cache object tree. Symbolic-link cache entries are never trusted as hits.
    pub fn committed_file_exists(&self, path: &Path) -> Result<bool> {
        let objects = self.path.join("cache/objects");
        if !path.starts_with(&objects) {
            return Err(GrapheneError::new(
                ErrorCode::CacheCommitFailed,
                ErrorKind::Storage,
                "cache candidate is outside the managed object tree",
            ));
        }

        let object_relative = ManagedRelativePath::new("cache/objects")?;
        let objects = ensure_managed_directory(&self.path, &object_relative)?;
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == IoErrorKind::NotFound => return Ok(false),
            Err(source) => {
                return Err(GrapheneError::new(
                    ErrorCode::FileOpenFailed,
                    ErrorKind::Storage,
                    "failed to inspect cache candidate",
                )
                .with_source(source));
            }
        };

        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Ok(false);
        }

        ensure_resolves_within(path, &objects, "cache candidate")?;
        Ok(true)
    }

    /// Commits a file that the caller has already fully written and verified.
    pub fn commit_verified(&self, temporary: &Path, destination: &Path) -> Result<()> {
        let temporary_root = self.path.join("cache/downloads/temporary");
        let object_root = self.path.join("cache/objects");
        if !temporary.starts_with(&temporary_root) || !destination.starts_with(&object_root) {
            return Err(GrapheneError::new(
                ErrorCode::CacheCommitFailed,
                ErrorKind::Storage,
                "cache commit paths are outside managed temporary/final areas",
            ));
        }

        let temporary_relative = ManagedRelativePath::new("cache/downloads/temporary")?;
        let object_relative = ManagedRelativePath::new("cache/objects")?;
        let temporary_root = ensure_managed_directory(&self.path, &temporary_relative)?;
        let object_root = ensure_managed_directory(&self.path, &object_relative)?;
        let temporary_metadata = fs::symlink_metadata(temporary).map_err(|source| {
            GrapheneError::new(
                ErrorCode::CacheCommitFailed,
                ErrorKind::Storage,
                "verified temporary cache object is unavailable",
            )
            .with_source(source)
        })?;

        if temporary_metadata.file_type().is_symlink() || !temporary_metadata.is_file() {
            return Err(GrapheneError::new(
                ErrorCode::CacheCommitFailed,
                ErrorKind::Storage,
                "cache commit source must be an ordinary managed file",
            ));
        }

        ensure_resolves_within(temporary, &temporary_root, "temporary cache object")?;
        let destination_parent = destination.parent().ok_or_else(|| {
            GrapheneError::new(
                ErrorCode::CacheCommitFailed,
                ErrorKind::Storage,
                "cache destination has no parent",
            )
        })?;

        let destination_relative = destination_parent.strip_prefix(&self.path).map_err(|_| {
            GrapheneError::new(
                ErrorCode::CacheCommitFailed,
                ErrorKind::Storage,
                "cache destination parent is outside the data root",
            )
        })?;

        let destination_relative = ManagedRelativePath::new(destination_relative)?;
        let resolved_parent = ensure_managed_directory(&self.path, &destination_relative)?;

        if !resolved_parent.starts_with(&object_root) {
            return Err(GrapheneError::new(
                ErrorCode::CacheCommitFailed,
                ErrorKind::Storage,
                "cache destination parent resolves outside the object tree",
            ));
        }

        replace_file_safely(temporary, destination).map_err(|error| {
            GrapheneError::new(
                ErrorCode::CacheCommitFailed,
                ErrorKind::Storage,
                "failed to commit verified cache object",
            )
            .with_context("destination", destination.display().to_string())
            .with_source(error)
        })
    }
}

fn verify_write_capability(root: &Path) -> Result<()> {
    let temporary_root = root.join("cache/downloads/temporary");
    let probe = temporary_root.join(format!(".graphene-write-probe-{}.tmp", ArtifactId::new()));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&probe)
            .map_err(|source| {
                GrapheneError::new(
                    ErrorCode::FileWriteFailed,
                    ErrorKind::Filesystem,
                    "Graphene data root is not writable",
                )
                .with_source(source)
            })?;

        file.write_all(b"graphene-write-probe").map_err(|source| {
            GrapheneError::new(
                ErrorCode::FileWriteFailed,
                ErrorKind::Filesystem,
                "failed to write Graphene data-root capability probe",
            )
            .with_source(source)
        })?;

        file.sync_all().map_err(|source| {
            GrapheneError::new(
                ErrorCode::FileWriteFailed,
                ErrorKind::Filesystem,
                "failed to sync Graphene data-root capability probe",
            )
            .with_source(source)
        })?;

        Ok(())
    })();

    let cleanup = fs::remove_file(&probe);
    if let Err(error) = result {
        let _ = cleanup;
        return Err(error);
    }

    cleanup.map_err(|source| {
        GrapheneError::new(
            ErrorCode::FileWriteFailed,
            ErrorKind::Filesystem,
            "failed to remove Graphene data-root capability probe",
        )
        .with_source(source)
    })
}

fn ensure_resolves_within(path: &Path, root: &Path, context: &'static str) -> Result<()> {
    let resolved_root = fs::canonicalize(root).map_err(|source| {
        GrapheneError::new(
            ErrorCode::DataRootInvalid,
            ErrorKind::Filesystem,
            "failed to canonicalize managed root",
        )
        .with_source(source)
    })?;
    let resolved = fs::canonicalize(path).map_err(|source| {
        GrapheneError::new(
            ErrorCode::DataRootInvalid,
            ErrorKind::Filesystem,
            "failed to canonicalize managed path",
        )
        .with_context("context", context)
        .with_source(source)
    })?;

    if !resolved.starts_with(&resolved_root) {
        return Err(GrapheneError::new(
            ErrorCode::DataRootInvalid,
            ErrorKind::Filesystem,
            "managed path resolves outside the Graphene data root",
        )
        .with_context("context", context));
    }

    Ok(())
}

#[cfg(test)]
mod tests;
