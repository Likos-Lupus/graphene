use crate::{DataRoot, atomic::write_new_atomic};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, ManagedRuntimeId, OperationId, Result};
use graphene_platform::{
    ManagedRelativePath, ensure_managed_directory, publish_directory_create_only,
};
use std::{fs, io::ErrorKind as IoErrorKind, path::PathBuf};

const MAX_RUNTIME_DESCRIPTOR_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone)]
pub struct ManagedRuntimeStore {
    root: PathBuf,
}

impl ManagedRuntimeStore {
    pub fn new(data_root: &DataRoot) -> Result<Self> {
        let root = ensure_managed_directory(
            data_root.path(),
            &ManagedRelativePath::new("shared/runtimes")?,
        )?;
        Ok(Self { root })
    }

    #[must_use]
    pub fn runtime_dir(&self, runtime_id: ManagedRuntimeId) -> PathBuf {
        self.root.join(runtime_id.to_string())
    }

    #[must_use]
    pub fn staging_dir(&self, operation_id: OperationId) -> PathBuf {
        self.root.join(format!(".staging-{operation_id}"))
    }

    pub fn list_ids(&self) -> Result<Vec<ManagedRuntimeId>> {
        let mut ids = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(runtime_io)? {
            let entry = entry.map_err(runtime_io)?;
            let ty = entry.file_type().map_err(runtime_io)?;
            if ty.is_symlink() || !ty.is_dir() {
                continue;
            }

            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };

            if name.starts_with(".staging-") {
                continue;
            }

            if let Ok(id) = name.parse() {
                ids.push(id);
            }
        }

        ids.sort_by_key(ToString::to_string);
        Ok(ids)
    }

    pub fn read_descriptor(&self, runtime_id: ManagedRuntimeId) -> Result<Option<Vec<u8>>> {
        let runtime_dir = self.runtime_dir(runtime_id);
        let runtime_metadata = match fs::symlink_metadata(&runtime_dir) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == IoErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(runtime_io(source)),
        };

        if runtime_metadata.file_type().is_symlink() || !runtime_metadata.is_dir() {
            return Err(GrapheneError::new(
                ErrorCode::JavaManagedRuntimeCorrupt,
                ErrorKind::Java,
                "managed runtime root is not an ordinary directory",
            ));
        }

        let path = runtime_dir.join("runtime.json");
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == IoErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(runtime_io(source)),
        };

        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > MAX_RUNTIME_DESCRIPTOR_BYTES
        {
            return Err(GrapheneError::new(
                ErrorCode::JavaManagedRuntimeCorrupt,
                ErrorKind::Java,
                "managed runtime descriptor is unsafe or oversized",
            ));
        }

        fs::read(path).map(Some).map_err(runtime_io)
    }

    pub fn prepare_staging(&self, operation_id: OperationId) -> Result<PathBuf> {
        let path = self.staging_dir(operation_id);
        match fs::create_dir(&path) {
            Ok(()) => Ok(path),
            Err(source) => Err(GrapheneError::new(
                ErrorCode::JavaManagedInstallFailed,
                ErrorKind::Storage,
                "failed to create managed Java staging directory",
            )
            .with_source(source)),
        }
    }

    pub fn payload_dir(&self, operation_id: OperationId) -> Result<PathBuf> {
        let path = self.staging_dir(operation_id).join("runtime");
        fs::create_dir(&path).map_err(|source| {
            GrapheneError::new(
                ErrorCode::JavaManagedInstallFailed,
                ErrorKind::Storage,
                "failed to create managed runtime payload directory",
            )
            .with_source(source)
        })?;
        Ok(path)
    }

    pub fn write_staging_descriptor(&self, operation_id: OperationId, bytes: &[u8]) -> Result<()> {
        if bytes.len() as u64 > MAX_RUNTIME_DESCRIPTOR_BYTES {
            return Err(GrapheneError::new(
                ErrorCode::JavaManagedRuntimeCorrupt,
                ErrorKind::Storage,
                "managed runtime descriptor exceeds its size bound",
            ));
        }

        write_new_atomic(&self.staging_dir(operation_id).join("runtime.json"), bytes).map_err(
            |error| {
                GrapheneError::new(
                    ErrorCode::JavaManagedInstallFailed,
                    ErrorKind::Storage,
                    "failed to persist managed runtime descriptor in staging",
                )
                .with_source(error)
            },
        )
    }

    pub fn cleanup_staging(&self, operation_id: OperationId) -> Result<()> {
        let path = self.staging_dir(operation_id);
        match fs::remove_dir_all(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == IoErrorKind::NotFound => Ok(()),
            Err(source) => Err(runtime_io(source)),
        }
    }

    pub fn publish(&self, operation_id: OperationId, runtime_id: ManagedRuntimeId) -> Result<()> {
        let staging = self.staging_dir(operation_id);
        let destination = self.runtime_dir(runtime_id);
        publish_directory_create_only(&staging, &destination).map_err(|source| {
            GrapheneError::new(
                ErrorCode::JavaManagedInstallFailed,
                ErrorKind::Storage,
                "failed to publish managed Java runtime create-only",
            )
            .with_source(source)
        })
    }
}

fn runtime_io(source: std::io::Error) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::JavaManagedRuntimeCorrupt,
        ErrorKind::Storage,
        "managed runtime storage operation failed",
    )
    .with_source(source)
}
