use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use std::{
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
};

/// Validated relative path that cannot escape a Graphene-managed root via absolute components or
/// `..` traversal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ManagedRelativePath(PathBuf);

impl ManagedRelativePath {
    /// Validates a managed relative path.
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if path.as_os_str().is_empty() {
            return Err(invalid_path("managed path is empty"));
        }

        for component in path.components() {
            match component {
                Component::Normal(component) => validate_component(component)?,
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(invalid_path(
                        "managed relative path contains an escaping component",
                    ));
                }
            }
        }

        Ok(Self(path.to_path_buf()))
    }

    /// Returns the validated relative path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Joins the managed relative path under a root.
    #[must_use]
    pub fn under(&self, root: &Path) -> PathBuf {
        root.join(&self.0)
    }
}

/// Publishes a fully staged directory to a destination that must not already exist.
///
/// On Linux/Android and macOS this uses the operating system's no-replace rename primitive so the
/// create-only condition and publication are one filesystem operation. Windows directory rename is
/// itself no-replace. Other targets retain the same-process install lock invariant and perform a
/// final existence check immediately before rename.
pub fn publish_directory_create_only(staging: &Path, destination: &Path) -> std::io::Result<()> {
    let source_metadata = fs::symlink_metadata(staging)?;
    if source_metadata.file_type().is_symlink() || !source_metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "staging publication source must be an ordinary directory",
        ));
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let source = CString::new(staging.as_os_str().as_bytes()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "staging path contains NUL",
            )
        })?;
        let target = CString::new(destination.as_os_str().as_bytes()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "destination path contains NUL",
            )
        })?;
        // SAFETY: both pointers are valid NUL-terminated path buffers for the duration of the call.
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                source.as_ptr(),
                libc::AT_FDCWD,
                target.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        return if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        };
    }

    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let source = CString::new(staging.as_os_str().as_bytes()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "staging path contains NUL",
            )
        })?;
        let target = CString::new(destination.as_os_str().as_bytes()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "destination path contains NUL",
            )
        })?;
        // SAFETY: both pointers are valid NUL-terminated path buffers for the duration of the call.
        let result =
            unsafe { libc::renamex_np(source.as_ptr(), target.as_ptr(), libc::RENAME_EXCL) };
        return if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        };
    }

    #[cfg(windows)]
    {
        // std::fs::rename on Windows fails when the destination directory already exists.
        return fs::rename(staging, destination);
    }

    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        windows
    )))]
    {
        if fs::symlink_metadata(destination).is_ok() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "destination already exists",
            ));
        }
        fs::rename(staging, destination)
    }
}

/// Converts a user-provided root to an absolute path without relying on global Graphene state.
pub fn normalize_root(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    if path.as_os_str().is_empty() {
        return Err(GrapheneError::new(
            ErrorCode::DataRootInvalid,
            ErrorKind::Configuration,
            "data root must not be empty",
        ));
    }

    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|source| {
                GrapheneError::new(
                    ErrorCode::DataRootInvalid,
                    ErrorKind::Filesystem,
                    "failed to resolve relative data root",
                )
                .with_source(source)
            })
    }
}

fn validate_component(component: &OsStr) -> Result<()> {
    if component.as_encoded_bytes().contains(&0) {
        return Err(invalid_path("managed path contains a NUL byte"));
    }

    #[cfg(windows)]
    {
        let component = component.to_string_lossy();
        if component.ends_with(' ') || component.ends_with('.') {
            return Err(invalid_path(
                "managed path contains a Windows-invalid trailing character",
            ));
        }

        if component.chars().any(|character| {
            character <= '\u{1f}' || matches!(character, '<' | '>' | ':' | '"' | '|' | '?' | '*')
        }) {
            return Err(invalid_path(
                "managed path contains a Windows-invalid character",
            ));
        }

        let stem = component
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || stem
                .strip_prefix("COM")
                .or_else(|| stem.strip_prefix("LPT"))
                .is_some_and(|suffix| {
                    matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
                });
        if reserved {
            return Err(invalid_path(
                "managed path contains a reserved Windows device name",
            ));
        }
    }

    Ok(())
}

fn invalid_path(message: &'static str) -> GrapheneError {
    GrapheneError::new(ErrorCode::DataRootInvalid, ErrorKind::Filesystem, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_paths_reject_escape_and_absolute_paths() {
        assert!(ManagedRelativePath::new("../outside").is_err());
        assert!(ManagedRelativePath::new("/absolute").is_err());
        assert!(ManagedRelativePath::new("cache/objects/file").is_ok());
    }
    #[cfg(unix)]
    #[test]
    fn managed_paths_reject_nul_components() {
        use std::os::unix::ffi::OsStringExt;

        let path = std::ffi::OsString::from_vec(b"cache/bad\0name".to_vec());
        assert!(ManagedRelativePath::new(PathBuf::from(path)).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn managed_paths_reject_windows_invalid_components() {
        assert!(ManagedRelativePath::new("cache/CON/file").is_err());
        assert!(ManagedRelativePath::new("cache/bad?/file").is_err());
        assert!(ManagedRelativePath::new("cache/trailing./file").is_err());
        assert!(ManagedRelativePath::new("cache/bad\u{001f}/file").is_err());
    }
}
