use crate::ManagedRelativePath;
use fs2::FileExt;
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use std::{
    fs,
    io::ErrorKind as IoErrorKind,
    path::{Component, Path, PathBuf},
};

/// Creates a directory tree with structured Graphene-owned error conversion.
pub fn ensure_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|source| {
        GrapheneError::new(
            ErrorCode::DirectoryCreateFailed,
            ErrorKind::Filesystem,
            "failed to create managed directory",
        )
        .with_context("path", path.display().to_string())
        .with_source(source)
    })
}

/// Creates or validates a relative directory path below an existing root without traversing
/// symbolic-link components.
///
/// Components are handled one at a time. Existing symbolic links and non-directories are rejected,
/// and each resulting directory is canonicalized back under the canonical root before traversal
/// continues.
pub fn ensure_managed_directory(root: &Path, relative: &ManagedRelativePath) -> Result<PathBuf> {
    let canonical_root = fs::canonicalize(root).map_err(|source| {
        GrapheneError::new(
            ErrorCode::DataRootInvalid,
            ErrorKind::Filesystem,
            "failed to canonicalize managed directory root",
        )
        .with_source(source)
    })?;
    let mut current = canonical_root.clone();

    for component in relative.as_path().components() {
        let Component::Normal(name) = component else {
            continue;
        };

        let candidate = current.join(name);
        match fs::symlink_metadata(&candidate) {
            Ok(_) => {}
            Err(error) if error.kind() == IoErrorKind::NotFound => {
                match fs::create_dir(&candidate) {
                    Ok(()) => {}
                    Err(error) if error.kind() == IoErrorKind::AlreadyExists => {}
                    Err(source) => {
                        return Err(GrapheneError::new(
                            ErrorCode::DirectoryCreateFailed,
                            ErrorKind::Filesystem,
                            "failed to create managed directory component",
                        )
                        .with_context("path", candidate.display().to_string())
                        .with_source(source));
                    }
                }
            }

            Err(source) => {
                return Err(GrapheneError::new(
                    ErrorCode::DirectoryCreateFailed,
                    ErrorKind::Filesystem,
                    "failed to inspect managed directory component",
                )
                .with_context("path", candidate.display().to_string())
                .with_source(source));
            }
        }

        let metadata = fs::symlink_metadata(&candidate).map_err(|source| {
            GrapheneError::new(
                ErrorCode::DirectoryCreateFailed,
                ErrorKind::Filesystem,
                "failed to inspect managed directory after creation",
            )
            .with_context("path", candidate.display().to_string())
            .with_source(source)
        })?;

        if metadata.file_type().is_symlink() {
            return Err(GrapheneError::new(
                ErrorCode::DataRootInvalid,
                ErrorKind::Filesystem,
                "managed directory path contains a symbolic link",
            )
            .with_context("path", candidate.display().to_string()));
        }

        if !metadata.is_dir() {
            return Err(GrapheneError::new(
                ErrorCode::DirectoryCreateFailed,
                ErrorKind::Filesystem,
                "managed directory path contains a non-directory component",
            )
            .with_context("path", candidate.display().to_string()));
        }

        let resolved = fs::canonicalize(&candidate).map_err(|source| {
            GrapheneError::new(
                ErrorCode::DataRootInvalid,
                ErrorKind::Filesystem,
                "failed to canonicalize managed directory component",
            )
            .with_context("path", candidate.display().to_string())
            .with_source(source)
        })?;

        if !resolved.starts_with(&canonical_root) {
            return Err(GrapheneError::new(
                ErrorCode::DataRootInvalid,
                ErrorKind::Filesystem,
                "managed directory resolves outside its root",
            ));
        }
        current = resolved;
    }

    Ok(current)
}

/// Replaces `destination` with a completely written `source` while serializing cooperating writers
/// through an adjacent file lock.
///
/// On Unix, `rename` atomically replaces an existing file on one filesystem. On Windows, where a
/// direct replacement rename is not portable through `std`, the previous destination is moved to a
/// backup first and restored if the final rename fails. No partial source is copied into the final
/// path.
pub fn replace_file_safely(source: &Path, destination: &Path) -> Result<()> {
    let parent = destination.parent().ok_or_else(|| {
        GrapheneError::new(
            ErrorCode::FileRenameFailed,
            ErrorKind::Filesystem,
            "destination has no parent directory",
        )
    })?;
    ensure_directory(parent)?;

    let file_name = destination
        .file_name()
        .ok_or_else(|| {
            GrapheneError::new(
                ErrorCode::FileRenameFailed,
                ErrorKind::Filesystem,
                "destination has no file name",
            )
        })?
        .to_string_lossy();
    let lock_path = parent.join(format!(".{file_name}.graphene-lock"));
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|source| {
            GrapheneError::new(
                ErrorCode::FileOpenFailed,
                ErrorKind::Filesystem,
                "failed to open replacement lock",
            )
            .with_source(source)
        })?;
    lock_file.lock_exclusive().map_err(|source| {
        GrapheneError::new(
            ErrorCode::FileOpenFailed,
            ErrorKind::Filesystem,
            "failed to lock replacement destination",
        )
        .with_source(source)
    })?;

    let result = replace_locked(source, destination);
    let _ = FileExt::unlock(&lock_file);
    result
}

#[cfg(unix)]
fn replace_locked(source: &Path, destination: &Path) -> Result<()> {
    fs::rename(source, destination).map_err(|source_error| {
        GrapheneError::new(
            ErrorCode::FileRenameFailed,
            ErrorKind::Filesystem,
            "failed to atomically replace destination",
        )
        .with_context("destination", destination.display().to_string())
        .with_source(source_error)
    })
}

#[cfg(windows)]
fn replace_locked(source: &Path, destination: &Path) -> Result<()> {
    if !destination.exists() {
        return fs::rename(source, destination).map_err(|source_error| {
            GrapheneError::new(
                ErrorCode::FileRenameFailed,
                ErrorKind::Filesystem,
                "failed to commit destination",
            )
            .with_source(source_error)
        });
    }

    let file_name = destination
        .file_name()
        .expect("destination file name validated")
        .to_string_lossy();
    let backup = destination.with_file_name(format!(".{file_name}.graphene-replace-backup"));
    let _ = fs::remove_file(&backup);

    fs::rename(destination, &backup).map_err(|source_error| {
        GrapheneError::new(
            ErrorCode::FileRenameFailed,
            ErrorKind::Filesystem,
            "failed to stage previous destination for replacement",
        )
        .with_source(source_error)
    })?;

    match fs::rename(source, destination) {
        Ok(()) => {
            let _ = fs::remove_file(backup);
            Ok(())
        }

        Err(source_error) => {
            let restore_result = fs::rename(&backup, destination);
            let mut error = GrapheneError::new(
                ErrorCode::FileRenameFailed,
                ErrorKind::Filesystem,
                "failed to replace destination; previous file restoration attempted",
            )
            .with_source(source_error);
            if restore_result.is_err() {
                error = error.with_context("restore_failed", "true");
            }

            Err(error)
        }
    }
}

#[cfg(not(any(unix, windows)))]
fn replace_locked(_source: &Path, _destination: &Path) -> Result<()> {
    Err(GrapheneError::new(
        ErrorCode::PlatformUnsupported,
        ErrorKind::Platform,
        "safe file replacement is unsupported on this target",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("graphene-platform-{name}-{unique}"));
        fs::create_dir_all(&path).expect("temp dir");
        path
    }

    #[test]
    fn managed_directory_creation_is_component_safe() {
        let dir = temp_dir("managed-directory");
        let relative = ManagedRelativePath::new("cache/objects/sha256").expect("relative");
        let created = ensure_managed_directory(&dir, &relative).expect("managed directories");

        assert!(created.is_dir());
        assert!(created.starts_with(fs::canonicalize(&dir).expect("canonical root")));
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn managed_directory_creation_rejects_intermediate_symlinks_without_writing_through_them() {
        use std::os::unix::fs::symlink;

        let dir = temp_dir("managed-symlink");
        let outside = temp_dir("managed-symlink-outside");
        let cache = dir.join("cache");
        fs::create_dir(&cache).expect("cache");
        symlink(&outside, cache.join("objects")).expect("symlink");
        let relative = ManagedRelativePath::new("cache/objects/created-outside").expect("relative");

        let error = ensure_managed_directory(&dir, &relative).expect_err("symlink must fail");
        assert_eq!(error.code, ErrorCode::DataRootInvalid);
        assert!(!outside.join("created-outside").exists());
        let _ = fs::remove_dir_all(dir);
        let _ = fs::remove_dir_all(outside);
    }

    #[test]
    fn safe_replacement_preserves_complete_file_semantics() {
        let dir = temp_dir("replace");
        let source = dir.join("source.part");
        let destination = dir.join("final");
        fs::write(&destination, b"old").expect("old");
        fs::write(&source, b"new-complete").expect("new");
        replace_file_safely(&source, &destination).expect("replace");
        assert_eq!(fs::read(&destination).expect("final"), b"new-complete");
        assert!(!source.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn failed_replacement_does_not_report_success() {
        let dir = temp_dir("failure");
        let missing = dir.join("missing.part");
        let destination = dir.join("final");
        fs::write(&destination, b"previous-valid").expect("previous");
        let error = replace_file_safely(&missing, &destination).expect_err("must fail");
        assert_eq!(error.code, ErrorCode::FileRenameFailed);
        assert_eq!(
            fs::read(&destination).expect("previous survives"),
            b"previous-valid"
        );
        let _ = fs::remove_dir_all(dir);
    }
}
