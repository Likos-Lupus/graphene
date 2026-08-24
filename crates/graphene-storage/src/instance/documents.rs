use crate::atomic::write_new_atomic;
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use serde::{Serialize, de::DeserializeOwned};
use std::{fs, path::Path};

/// Maximum byte limit for `instance.json` (64 KiB).
pub const MAX_DESCRIPTOR_BYTES: usize = 64 * 1024;

/// Maximum byte limit for `install.json` (512 KiB).
pub const MAX_RECEIPT_BYTES: usize = 512 * 1024;

/// Maximum byte limit for `lock.json` (4 MiB).
pub const MAX_LOCKFILE_BYTES: usize = 4 * 1024 * 1024;

/// Maximum byte limit for `config.json` and `instance-defaults.json` (64 KiB).
pub const MAX_CONFIG_BYTES: usize = 64 * 1024;

/// Reads raw bytes of an instance document with strict size bounds and symbolic-link rejection.
pub fn read_document_bounded(path: &Path, max_bytes: usize) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|source| {
        let code = if source.kind() == std::io::ErrorKind::NotFound {
            ErrorCode::InstanceNotFound
        } else {
            ErrorCode::FileOpenFailed
        };
        GrapheneError::new(
            code,
            ErrorKind::Instance,
            "failed to inspect document metadata",
        )
        .with_context("path", path.display().to_string())
        .with_source(source)
    })?;

    if metadata.file_type().is_symlink() {
        return Err(GrapheneError::new(
            ErrorCode::InstanceInvalid,
            ErrorKind::Instance,
            "instance document path is a symbolic link",
        )
        .with_context("path", path.display().to_string()));
    }

    if !metadata.is_file() {
        return Err(GrapheneError::new(
            ErrorCode::InstanceInvalid,
            ErrorKind::Instance,
            "instance document is not a regular file",
        )
        .with_context("path", path.display().to_string()));
    }

    if metadata.len() > max_bytes as u64 {
        return Err(GrapheneError::new(
            ErrorCode::InstanceInvalid,
            ErrorKind::Instance,
            "instance document exceeds maximum byte size limit",
        )
        .with_context("path", path.display().to_string())
        .with_context("size", metadata.len().to_string())
        .with_context("limit", max_bytes.to_string()));
    }

    fs::read(path).map_err(|source| {
        GrapheneError::new(
            ErrorCode::FileOpenFailed,
            ErrorKind::Instance,
            "failed to read instance document contents",
        )
        .with_context("path", path.display().to_string())
        .with_source(source)
    })
}

/// Atomically replaces or creates an instance document at `path`.
pub fn write_document_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    write_new_atomic(path, bytes)
}

/// Reads and deserializes a JSON document with strict size bounds and symbolic-link rejection.
pub fn read_json_bounded<T: DeserializeOwned>(path: &Path, max_bytes: usize) -> Result<T> {
    let bytes = read_document_bounded(path, max_bytes)?;
    serde_json::from_slice(&bytes).map_err(|source| {
        GrapheneError::new(
            ErrorCode::InstanceInvalid,
            ErrorKind::Instance,
            "failed to deserialize instance JSON document",
        )
        .with_context("path", path.display().to_string())
        .with_source(source)
    })
}

/// Serializes a JSON document pretty-printed and writes it atomically to `path`.
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|source| {
        GrapheneError::new(
            ErrorCode::InstanceMutationFailed,
            ErrorKind::Instance,
            "failed to serialize instance document to JSON",
        )
        .with_context("path", path.display().to_string())
        .with_source(source)
    })?;
    write_document_atomic(path, &bytes)
}
