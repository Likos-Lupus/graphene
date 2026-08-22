use graphene_core::{ArtifactId, ErrorCode, ErrorKind, GrapheneError, Result};
use graphene_platform::replace_file_safely;
use std::{fs, io::Write, path::Path};

pub(crate) fn write_new_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        GrapheneError::new(
            ErrorCode::FileWriteFailed,
            ErrorKind::Filesystem,
            "atomic write destination has no parent",
        )
    })?;
    graphene_platform::ensure_directory(parent)?;
    let file_name = path.file_name().ok_or_else(|| {
        GrapheneError::new(
            ErrorCode::FileWriteFailed,
            ErrorKind::Filesystem,
            "atomic write destination has no file name",
        )
    })?;
    let temp = parent.join(format!(
        ".{}.graphene-write-{}.part",
        file_name.to_string_lossy(),
        ArtifactId::new()
    ));

    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)
        .map_err(|source| {
            GrapheneError::new(
                ErrorCode::FileOpenFailed,
                ErrorKind::Filesystem,
                "failed to create temporary atomic write",
            )
            .with_source(source)
        })?;
    if let Err(source) = file
        .write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
    {
        let _ = fs::remove_file(&temp);
        return Err(GrapheneError::new(
            ErrorCode::FileWriteFailed,
            ErrorKind::Filesystem,
            "failed to complete temporary atomic write",
        )
        .with_source(source));
    }

    drop(file);
    if let Err(error) = replace_file_safely(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }

    Ok(())
}
