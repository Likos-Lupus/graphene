use super::{
    JavaService,
    support::{java_error, spawn_blocking_java},
};
use graphene_core::{ErrorCode, Result};
use graphene_java::{
    JavaArchitecture, JavaCandidate, JavaCandidateSource, JavaRequirement, JavaRuntime,
    ManagedJavaRuntime, is_compatible, probe_java,
};
use graphene_storage::ManagedRuntimeStore;
use std::{
    fs,
    path::{Path, PathBuf},
};

impl JavaService {
    pub(super) async fn load_one_runtime(
        &self,
        id: graphene_core::ManagedRuntimeId,
    ) -> Result<Option<ManagedJavaRuntime>> {
        let root = self.context.storage.clone();
        spawn_blocking_java(move || {
            let store = ManagedRuntimeStore::new(&root)?;
            let Some(bytes) = store.read_descriptor(id)? else {
                return Ok(None);
            };

            let descriptor: ManagedJavaRuntime =
                serde_json::from_slice(&bytes).map_err(|source| {
                    java_error(
                        ErrorCode::JavaManagedRuntimeCorrupt,
                        "managed runtime descriptor JSON is malformed",
                    )
                    .with_source(source)
                })?;

            descriptor.validate()?;
            Ok(Some(descriptor))
        })
        .await
    }
}

pub(super) fn load_inventory(root: &graphene_storage::DataRoot) -> Result<Vec<ManagedJavaRuntime>> {
    let store = ManagedRuntimeStore::new(root)?;
    let mut runtimes = Vec::new();

    for id in store.list_ids()? {
        let Some(bytes) = store.read_descriptor(id)? else {
            continue;
        };

        let descriptor: ManagedJavaRuntime = serde_json::from_slice(&bytes).map_err(|source| {
            java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "managed runtime descriptor JSON is malformed",
            )
            .with_source(source)
        })?;
        descriptor.validate()?;
        if descriptor.id != id {
            return Err(java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "managed runtime descriptor identity mismatches its directory",
            ));
        }

        validate_managed_executable_path(
            &store.runtime_dir(id),
            &descriptor.executable_relative_path,
        )?;
        runtimes.push(descriptor);
    }

    runtimes.sort_by_key(|runtime| runtime.id.to_string());
    Ok(runtimes)
}

pub(super) async fn runtime_dir_exists(path: PathBuf) -> Result<bool> {
    spawn_blocking_java(move || match fs::symlink_metadata(&path) {
        Ok(metadata) => Ok(metadata.is_dir() && !metadata.file_type().is_symlink()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(java_error(
            ErrorCode::JavaManagedRuntimeCorrupt,
            "failed to inspect managed runtime directory",
        )
        .with_source(source)),
    })
    .await
}

pub(super) async fn validate_and_probe_committed(
    descriptor: ManagedJavaRuntime,
    runtime_dir: PathBuf,
    requirement: &JavaRequirement,
) -> Result<JavaRuntime> {
    descriptor.validate()?;
    let relative = descriptor.executable_relative_path.clone();
    let validation_root = runtime_dir.clone();
    spawn_blocking_java(move || validate_managed_executable_path(&validation_root, &relative))
        .await?;
    let expected = descriptor.to_java_runtime(&runtime_dir)?;
    let candidate = JavaCandidate {
        executable: expected.executable.clone(),
        source: JavaCandidateSource::Managed,
    };
    let probed = probe_java(&candidate).await.map_err(|source| {
        java_error(
            ErrorCode::JavaManagedRuntimeCorrupt,
            "committed managed Java runtime no longer probes successfully",
        )
        .with_source(source)
    })?;

    if probed.major_version != descriptor.major_version
        || probed.architecture != descriptor.architecture
        || probed.version != descriptor.version
        || probed.vendor != descriptor.vendor
        || !is_compatible(&probed, requirement, JavaArchitecture::current())
    {
        return Err(java_error(
            ErrorCode::JavaManagedRuntimeCorrupt,
            "committed managed Java runtime probe differs from its descriptor",
        ));
    }

    Ok(probed)
}

fn validate_managed_executable_path(runtime_dir: &Path, relative: &str) -> Result<PathBuf> {
    let root_metadata = fs::symlink_metadata(runtime_dir).map_err(|source| {
        java_error(
            ErrorCode::JavaManagedRuntimeCorrupt,
            "managed runtime root is unavailable",
        )
        .with_source(source)
    })?;

    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(java_error(
            ErrorCode::JavaManagedRuntimeCorrupt,
            "managed runtime root is not an ordinary directory",
        ));
    }

    let mut current = runtime_dir.to_path_buf();
    let components: Vec<_> = Path::new(relative).components().collect();
    if components.is_empty() {
        return Err(java_error(
            ErrorCode::JavaManagedRuntimeCorrupt,
            "managed runtime executable path is empty",
        ));
    }

    for (index, component) in components.iter().enumerate() {
        use std::path::Component;
        let Component::Normal(part) = component else {
            return Err(java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "managed runtime executable path is unsafe",
            ));
        };

        current.push(part);
        let metadata = fs::symlink_metadata(&current).map_err(|source| {
            java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "managed runtime executable path is unavailable",
            )
            .with_source(source)
        })?;
        if metadata.file_type().is_symlink() {
            return Err(java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "managed runtime executable path contains a symbolic link",
            ));
        }

        let is_last = index + 1 == components.len();
        if (is_last && !metadata.is_file()) || (!is_last && !metadata.is_dir()) {
            return Err(java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "managed runtime executable path has an invalid entry type",
            ));
        }
    }

    Ok(current)
}

pub(super) fn locate_java_executable(root: &Path) -> Result<PathBuf> {
    const MAX_SCAN_ENTRIES: usize = 4096;

    let expected_name = if cfg!(windows) { "java.exe" } else { "java" };
    let mut stack = vec![root.to_path_buf()];
    let mut matches = Vec::new();
    let mut seen = 0usize;

    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(&directory).map_err(|source| {
            java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "failed to inspect staged managed runtime",
            )
            .with_source(source)
        })? {
            let entry = entry.map_err(|source| {
                java_error(
                    ErrorCode::JavaManagedRuntimeCorrupt,
                    "failed to inspect staged managed runtime entry",
                )
                .with_source(source)
            })?;
            seen += 1;
            if seen > MAX_SCAN_ENTRIES {
                return Err(java_error(
                    ErrorCode::JavaManagedRuntimeCorrupt,
                    "staged managed runtime contains too many filesystem entries",
                ));
            }

            let ty = entry.file_type().map_err(|source| {
                java_error(
                    ErrorCode::JavaManagedRuntimeCorrupt,
                    "failed to inspect staged runtime entry type",
                )
                .with_source(source)
            })?;

            if ty.is_symlink() {
                return Err(java_error(
                    ErrorCode::JavaManagedRuntimeCorrupt,
                    "staged managed runtime contains a symbolic link",
                ));
            }

            if ty.is_dir() {
                stack.push(entry.path());
            } else if ty.is_file()
                && entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(expected_name)
            {
                let path = entry.path();
                if path
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == "bin")
                {
                    matches.push(path);
                }
            }
        }
    }

    matches.sort();
    matches.into_iter().next().ok_or_else(|| {
        java_error(
            ErrorCode::JavaRuntimeUnexecutable,
            "managed runtime archive does not contain an expected bin/java executable",
        )
    })
}

pub(super) async fn make_executable(path: &Path) -> Result<()> {
    let path = path.to_path_buf();
    spawn_blocking_java(move || {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::symlink_metadata(&path).map_err(|source| {
                java_error(
                    ErrorCode::JavaRuntimeUnexecutable,
                    "failed to inspect staged Java executable permissions",
                )
                .with_source(source)
            })?;

            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(java_error(
                    ErrorCode::JavaRuntimeUnexecutable,
                    "staged Java executable is not an ordinary file",
                ));
            }

            let mut permissions = metadata.permissions();
            permissions.set_mode(permissions.mode() | 0o500);
            fs::set_permissions(&path, permissions).map_err(|source| {
                java_error(
                    ErrorCode::JavaRuntimeUnexecutable,
                    "failed to set staged Java executable permission",
                )
                .with_source(source)
            })?;
        }

        #[cfg(not(unix))]
        {
            let _ = path;
        }
        Ok(())
    })
    .await
}
