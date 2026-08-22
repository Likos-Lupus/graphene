use crate::ensure_directory;
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use std::{
    fs::{self, File, Metadata},
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Default buffer size for streamed I/O operations (64 KiB).
pub const STREAM_BUFFER_SIZE: usize = 64 * 1024;

/// Copies a single regular file in bounded stream chunks, syncing to disk.
pub fn copy_file_streamed(
    source: &Path,
    destination: &Path,
    max_bytes: u64,
    buf_size: usize,
) -> Result<u64> {
    let metadata = fs::symlink_metadata(source).map_err(|source_err| {
        GrapheneError::new(
            ErrorCode::FileOpenFailed,
            ErrorKind::Filesystem,
            "failed to inspect source file metadata",
        )
        .with_context("path", source.display().to_string())
        .with_source(source_err)
    })?;

    if metadata.file_type().is_symlink() {
        return Err(GrapheneError::new(
            ErrorCode::PlatformUnsupported,
            ErrorKind::Filesystem,
            "refusing to stream-copy symbolic link",
        )
        .with_context("path", source.display().to_string()));
    }

    if !metadata.is_file() {
        return Err(GrapheneError::new(
            ErrorCode::PlatformUnsupported,
            ErrorKind::Filesystem,
            "refusing to stream-copy non-regular file",
        )
        .with_context("path", source.display().to_string()));
    }

    if metadata.len() > max_bytes {
        return Err(GrapheneError::new(
            ErrorCode::FileWriteFailed,
            ErrorKind::Filesystem,
            "source file exceeds maximum byte size limit",
        )
        .with_context("path", source.display().to_string())
        .with_context("size", metadata.len().to_string())
        .with_context("limit", max_bytes.to_string()));
    }

    if let Some(parent) = destination.parent() {
        ensure_directory(parent)?;
    }

    let mut reader = File::open(source).map_err(|err| {
        GrapheneError::new(
            ErrorCode::FileOpenFailed,
            ErrorKind::Filesystem,
            "failed to open source file for reading",
        )
        .with_context("path", source.display().to_string())
        .with_source(err)
    })?;

    let mut writer = File::create(destination).map_err(|err| {
        GrapheneError::new(
            ErrorCode::FileWriteFailed,
            ErrorKind::Filesystem,
            "failed to create destination file for writing",
        )
        .with_context("path", destination.display().to_string())
        .with_source(err)
    })?;

    let mut buffer = vec![0u8; buf_size.max(4096)];
    let mut total_written: u64 = 0;

    loop {
        let bytes_read = reader.read(&mut buffer).map_err(|err| {
            GrapheneError::new(
                ErrorCode::FileOpenFailed,
                ErrorKind::Filesystem,
                "failed to read from source file",
            )
            .with_context("path", source.display().to_string())
            .with_source(err)
        })?;

        if bytes_read == 0 {
            break;
        }

        total_written = total_written
            .checked_add(bytes_read as u64)
            .ok_or_else(|| {
                GrapheneError::new(
                    ErrorCode::FileWriteFailed,
                    ErrorKind::Filesystem,
                    "file size overflow during copy",
                )
            })?;

        if total_written > max_bytes {
            return Err(GrapheneError::new(
                ErrorCode::FileWriteFailed,
                ErrorKind::Filesystem,
                "streamed file copy exceeded maximum byte size limit",
            )
            .with_context("path", source.display().to_string())
            .with_context("total", total_written.to_string())
            .with_context("limit", max_bytes.to_string()));
        }

        writer.write_all(&buffer[..bytes_read]).map_err(|err| {
            GrapheneError::new(
                ErrorCode::FileWriteFailed,
                ErrorKind::Filesystem,
                "failed to write to destination file",
            )
            .with_context("path", destination.display().to_string())
            .with_source(err)
        })?;
    }

    writer.flush().map_err(|err| {
        GrapheneError::new(
            ErrorCode::FileWriteFailed,
            ErrorKind::Filesystem,
            "failed to flush destination file",
        )
        .with_source(err)
    })?;

    writer.sync_all().map_err(|err| {
        GrapheneError::new(
            ErrorCode::FileWriteFailed,
            ErrorKind::Filesystem,
            "failed to sync destination file to storage",
        )
        .with_source(err)
    })?;

    Ok(total_written)
}

/// Recursively traverses a directory without following symbolic links.
///
/// Rejects any symbolic links or special non-file/non-directory objects (FIFOs, sockets, devices).
pub fn walk_contained_tree<F>(root: &Path, visitor: &mut F) -> Result<()>
where
    F: FnMut(&Path, &Metadata) -> Result<()>,
{
    let metadata = fs::symlink_metadata(root).map_err(|source| {
        GrapheneError::new(
            ErrorCode::FileOpenFailed,
            ErrorKind::Filesystem,
            "failed to inspect directory root metadata",
        )
        .with_context("path", root.display().to_string())
        .with_source(source)
    })?;

    if metadata.file_type().is_symlink() {
        return Err(GrapheneError::new(
            ErrorCode::PlatformUnsupported,
            ErrorKind::Filesystem,
            "managed tree traversal encountered a symbolic link at root",
        )
        .with_context("path", root.display().to_string()));
    }

    if !metadata.is_dir() {
        return Err(GrapheneError::new(
            ErrorCode::PlatformUnsupported,
            ErrorKind::Filesystem,
            "managed tree traversal root is not a directory",
        )
        .with_context("path", root.display().to_string()));
    }

    walk_dir_inner(root, visitor)
}

fn walk_dir_inner<F>(dir: &Path, visitor: &mut F) -> Result<()>
where
    F: FnMut(&Path, &Metadata) -> Result<()>,
{
    let entries = fs::read_dir(dir).map_err(|source| {
        GrapheneError::new(
            ErrorCode::FileOpenFailed,
            ErrorKind::Filesystem,
            "failed to read directory entries",
        )
        .with_context("path", dir.display().to_string())
        .with_source(source)
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| {
            GrapheneError::new(
                ErrorCode::FileOpenFailed,
                ErrorKind::Filesystem,
                "failed to access directory entry",
            )
            .with_source(source)
        })?;

        let entry_path = entry.path();
        let metadata = fs::symlink_metadata(&entry_path).map_err(|source| {
            GrapheneError::new(
                ErrorCode::FileOpenFailed,
                ErrorKind::Filesystem,
                "failed to inspect entry metadata",
            )
            .with_context("path", entry_path.display().to_string())
            .with_source(source)
        })?;

        if metadata.file_type().is_symlink() {
            return Err(GrapheneError::new(
                ErrorCode::PlatformUnsupported,
                ErrorKind::Filesystem,
                "managed directory contains forbidden symbolic link",
            )
            .with_context("path", entry_path.display().to_string()));
        }

        let is_dir = metadata.is_dir();
        let is_file = metadata.is_file();

        if !is_dir && !is_file {
            return Err(GrapheneError::new(
                ErrorCode::PlatformUnsupported,
                ErrorKind::Filesystem,
                "managed directory contains forbidden special file",
            )
            .with_context("path", entry_path.display().to_string()));
        }

        visitor(&entry_path, &metadata)?;

        if is_dir {
            walk_dir_inner(&entry_path, visitor)?;
        }
    }

    Ok(())
}

/// Recursively copies all regular files and directories from `source_root` into `dest_root`.
///
/// Ensures no symbolic links or special files are traversed. All regular files are streamed and
/// synced. Total bytes copied are bounded by `max_total_bytes`.
pub fn copy_contained_tree(
    source_root: &Path,
    dest_root: &Path,
    max_total_bytes: u64,
) -> Result<u64> {
    ensure_directory(dest_root)?;

    let mut total_bytes: u64 = 0;
    let mut files_to_copy: Vec<(PathBuf, PathBuf, u64)> = Vec::new();
    let mut dirs_to_create: Vec<PathBuf> = Vec::new();

    walk_contained_tree(source_root, &mut |path, metadata| {
        let relative = path.strip_prefix(source_root).map_err(|_| {
            GrapheneError::new(
                ErrorCode::DataRootInvalid,
                ErrorKind::Filesystem,
                "path is not contained within source root",
            )
        })?;

        let dest_path = dest_root.join(relative);

        if metadata.is_dir() {
            dirs_to_create.push(dest_path);
        } else if metadata.is_file() {
            let file_size = metadata.len();
            total_bytes = total_bytes.checked_add(file_size).ok_or_else(|| {
                GrapheneError::new(
                    ErrorCode::FileWriteFailed,
                    ErrorKind::Filesystem,
                    "tree size overflow during copy calculation",
                )
            })?;

            if total_bytes > max_total_bytes {
                return Err(GrapheneError::new(
                    ErrorCode::FileWriteFailed,
                    ErrorKind::Filesystem,
                    "tree copy exceeds maximum total byte limit",
                )
                .with_context("total", total_bytes.to_string())
                .with_context("limit", max_total_bytes.to_string()));
            }

            files_to_copy.push((path.to_path_buf(), dest_path, file_size));
        }

        Ok(())
    })?;

    for dir in dirs_to_create {
        ensure_directory(&dir)?;
    }

    for (source_file, dest_file, _) in files_to_copy {
        copy_file_streamed(
            &source_file,
            &dest_file,
            max_total_bytes,
            STREAM_BUFFER_SIZE,
        )?;
    }

    Ok(total_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streamed_copy_and_tree_copy_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");

        let sub_dir = src.join("sub");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(src.join("hello.txt"), b"hello world").unwrap();
        fs::write(sub_dir.join("sub.txt"), b"sub content").unwrap();

        let bytes = copy_contained_tree(&src, &dst, 1024 * 1024).unwrap();
        assert_eq!(bytes, 22);

        assert_eq!(fs::read(dst.join("hello.txt")).unwrap(), b"hello world");
        assert_eq!(fs::read(dst.join("sub/sub.txt")).unwrap(), b"sub content");
    }

    #[test]
    fn tree_copy_rejects_exceeded_byte_limit() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");

        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("big.bin"), vec![0u8; 1000]).unwrap();

        let err = copy_contained_tree(&src, &dst, 500).unwrap_err();
        assert_eq!(err.code, ErrorCode::FileWriteFailed);
    }
}
