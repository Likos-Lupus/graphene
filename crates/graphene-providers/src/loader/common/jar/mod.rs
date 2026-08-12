use graphene_core::{
    CancellationToken, ErrorCode, ErrorKind, GrapheneError, Result,
    archive::{ArchiveCodecError, central_entries, crc32, extract_entry},
};
use std::{collections::BTreeMap, fs, path::Path};

pub(super) const MAX_INSTALLER_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
pub(super) const MAX_INSTALLER_ENTRIES: usize = 8192;
pub(crate) const MAX_INSTALLER_ENTRY_BYTES: usize = 32 * 1024 * 1024;
pub(super) const MAX_INSTALLER_NAME_BYTES: usize = 1024;
const MAX_TOTAL_SELECTED_BYTES: usize = 64 * 1024 * 1024;
const MAX_EXPANSION_RATIO: usize = 1024;

/// Reads only explicitly selected regular entries from an already verified installer JAR.
pub(crate) fn read_selected_entries(
    archive: &Path,
    names: &[&str],
    cancellation: &CancellationToken,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let metadata = fs::symlink_metadata(archive).map_err(|source| {
        installer_archive_error("failed to inspect verified loader installer").with_source(source)
    })?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_INSTALLER_ARCHIVE_BYTES
    {
        return Err(installer_archive_error(
            "verified loader installer is not a bounded ordinary file",
        ));
    }

    if names.len() > 512 {
        return Err(installer_archive_error(
            "too many loader installer entries were requested",
        ));
    }
    for name in names {
        validate_entry_name(name)?;
    }

    let bytes = fs::read(archive).map_err(|source| {
        installer_archive_error("failed to read verified loader installer").with_source(source)
    })?;
    let entries = central_entries(&bytes, MAX_INSTALLER_ENTRIES, MAX_INSTALLER_NAME_BYTES)
        .map_err(codec_error)?;
    let wanted = names
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut found = BTreeMap::new();
    let mut total = 0usize;

    for entry in entries {
        checkpoint(cancellation)?;
        if !wanted.contains(entry.name.as_str()) {
            continue;
        }

        validate_entry_name(&entry.name)?;
        if entry.is_directory || entry.is_symlink || !entry.is_regular {
            return Err(installer_archive_error(
                "selected loader installer entry is not a regular file",
            ));
        }

        if entry.uncompressed_size > MAX_INSTALLER_ENTRY_BYTES {
            return Err(installer_archive_error(
                "loader installer entry exceeds its size bound",
            ));
        }

        if entry.uncompressed_size > 0
            && (entry.compressed_size == 0
                || entry.uncompressed_size
                    > entry.compressed_size.saturating_mul(MAX_EXPANSION_RATIO))
        {
            return Err(installer_archive_error(
                "loader installer entry has an implausible expansion ratio",
            ));
        }

        total = total.checked_add(entry.uncompressed_size).ok_or_else(|| {
            installer_archive_error("loader installer selected-entry size accounting overflowed")
        })?;

        if total > MAX_TOTAL_SELECTED_BYTES {
            return Err(installer_archive_error(
                "selected loader installer entries exceed their total size bound",
            ));
        }

        let data = extract_entry(&bytes, &entry, MAX_INSTALLER_ENTRY_BYTES, cancellation)
            .map_err(codec_error)?;
        if crc32(&data) != entry.crc32 {
            return Err(installer_archive_error(
                "loader installer entry CRC does not match",
            ));
        }

        if found.insert(entry.name.clone(), data).is_some() {
            return Err(installer_archive_error(
                "loader installer contains a duplicate selected entry",
            ));
        }
    }

    Ok(found)
}

pub(crate) fn list_entry_names(
    archive: &Path,
    cancellation: &CancellationToken,
) -> Result<Vec<String>> {
    let metadata = fs::symlink_metadata(archive).map_err(|source| {
        installer_archive_error("failed to inspect verified loader installer").with_source(source)
    })?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_INSTALLER_ARCHIVE_BYTES
    {
        return Err(installer_archive_error(
            "verified loader installer is not a bounded ordinary file",
        ));
    }

    let bytes = fs::read(archive).map_err(|source| {
        installer_archive_error("failed to read verified loader installer").with_source(source)
    })?;
    let entries = central_entries(&bytes, MAX_INSTALLER_ENTRIES, MAX_INSTALLER_NAME_BYTES)
        .map_err(codec_error)?;
    let mut names = Vec::with_capacity(entries.len());

    for entry in entries {
        checkpoint(cancellation)?;
        validate_entry_name(&entry.name)?;
        if entry.is_symlink || (!entry.is_regular && !entry.is_directory) {
            return Err(installer_archive_error(
                "loader installer contains an unsafe entry type",
            ));
        }

        if entry.is_regular {
            if entry.uncompressed_size > MAX_INSTALLER_ENTRY_BYTES {
                return Err(installer_archive_error(
                    "loader installer entry exceeds its size bound",
                ));
            }

            if entry.uncompressed_size > 0
                && (entry.compressed_size == 0
                    || entry.uncompressed_size
                        > entry.compressed_size.saturating_mul(MAX_EXPANSION_RATIO))
            {
                return Err(installer_archive_error(
                    "loader installer entry has an implausible expansion ratio",
                ));
            }

            names.push(entry.name);
        }
    }
    Ok(names)
}

fn validate_entry_name(name: &str) -> Result<()> {
    let trimmed = name.trim_end_matches('/');
    if trimmed.is_empty()
        || name.len() > MAX_INSTALLER_NAME_BYTES
        || name.starts_with('/')
        || name.starts_with('\\')
        || name.contains('\\')
        || name.contains('\0')
    {
        return Err(installer_archive_error(
            "loader installer entry path is absolute or invalid",
        ));
    }

    if trimmed
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(installer_archive_error(
            "loader installer entry path contains traversal",
        ));
    }

    if trimmed
        .split('/')
        .next()
        .is_some_and(|part| part.contains(':'))
    {
        return Err(installer_archive_error(
            "loader installer entry path contains a platform prefix",
        ));
    }

    Ok(())
}

fn codec_error(source: ArchiveCodecError) -> GrapheneError {
    if source.is_cancelled() {
        GrapheneError::new(
            ErrorCode::LoaderProcessorCancelled,
            ErrorKind::Cancelled,
            "loader installer parsing was cancelled",
        )
    } else {
        installer_archive_error("loader installer ZIP structure or compression is invalid")
            .with_source(source)
    }
}

pub(super) fn installer_archive_error(message: &'static str) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::LoaderInstallerInvalid,
        ErrorKind::Minecraft,
        message,
    )
}

pub(super) fn checkpoint(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        Err(GrapheneError::new(
            ErrorCode::LoaderProcessorCancelled,
            ErrorKind::Cancelled,
            "loader installer parsing was cancelled",
        ))
    } else {
        Ok(())
    }
}
