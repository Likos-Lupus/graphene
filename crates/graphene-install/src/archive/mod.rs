mod deflate;
mod zip;

use self::zip::{central_entries, crc32, extract_entry};
use graphene_core::{CancellationToken, ErrorCode, ErrorKind, GrapheneError, Result};
use graphene_platform::{ManagedRelativePath, ensure_managed_directory};
use std::{fs, io::Write, path::Path};

const MAX_ARCHIVE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ENTRIES: usize = 4096;
const MAX_ENTRY_BYTES: usize = 64 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 256 * 1024 * 1024;
const MAX_NAME_BYTES: usize = 1024;
const MAX_EXPANSION_RATIO: usize = 1024;

pub(crate) fn extract_native_zip(
    archive: &Path,
    destination: &Path,
    cancellation: &CancellationToken,
) -> Result<()> {
    let metadata = fs::symlink_metadata(archive)
        .map_err(|source| archive_error("failed to inspect native archive").with_source(source))?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_ARCHIVE_BYTES
    {
        return Err(archive_error(
            "native archive is not a bounded ordinary file",
        ));
    }

    let bytes = fs::read(archive)
        .map_err(|source| archive_error("failed to read native archive").with_source(source))?;
    let entries = central_entries(&bytes)?;
    if entries.len() > MAX_ENTRIES {
        return Err(archive_error("native archive contains too many entries"));
    }

    let mut total = 0usize;
    for entry in entries {
        checkpoint(cancellation)?;
        if entry.name.starts_with("META-INF/") || entry.name == "META-INF" {
            continue;
        }

        if entry.uncompressed_size > MAX_ENTRY_BYTES {
            return Err(archive_error(
                "native archive entry exceeds the per-entry size limit",
            ));
        }

        if entry.uncompressed_size > 0
            && (entry.compressed_size == 0
                || entry.uncompressed_size
                    > entry.compressed_size.saturating_mul(MAX_EXPANSION_RATIO))
        {
            return Err(archive_error(
                "native archive entry has an implausible expansion ratio",
            ));
        }

        total = total
            .checked_add(entry.uncompressed_size)
            .ok_or_else(|| archive_error("native archive size accounting overflowed"))?;
        if total > MAX_TOTAL_BYTES {
            return Err(archive_error(
                "native archive exceeds the total decompression size limit",
            ));
        }

        let relative = validate_entry_name(&entry.name)?;
        if entry.is_directory {
            ensure_managed_directory(destination, &relative)?;
            continue;
        }

        if entry.is_symlink || !entry.is_regular {
            return Err(archive_error(
                "native archive contains an unsafe entry type",
            ));
        }

        let parent = relative
            .as_path()
            .parent()
            .unwrap_or_else(|| Path::new("."));
        if parent != Path::new("") && parent != Path::new(".") {
            let parent = ManagedRelativePath::new(parent)?;
            ensure_managed_directory(destination, &parent)?;
        }

        let data = extract_entry(&bytes, &entry, cancellation)?;
        if crc32(&data) != entry.crc32 {
            return Err(archive_error("native archive entry CRC does not match"));
        }

        let target = relative.under(destination);
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&target)
            .map_err(|source| {
                archive_error("failed to create extracted native file").with_source(source)
            })?;
        file.write_all(&data).map_err(|source| {
            archive_error("failed to write extracted native file").with_source(source)
        })?;
        file.sync_all().map_err(|source| {
            archive_error("failed to sync extracted native file").with_source(source)
        })?;
    }
    Ok(())
}

fn validate_entry_name(name: &str) -> Result<ManagedRelativePath> {
    let trimmed = name.trim_end_matches('/');
    if trimmed.is_empty() || name.starts_with('/') || name.starts_with('\\') || name.contains('\\')
    {
        return Err(archive_error(
            "native archive entry path is absolute or empty",
        ));
    }

    if trimmed
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(archive_error(
            "native archive entry path contains traversal",
        ));
    }

    if trimmed
        .split('/')
        .next()
        .is_some_and(|part| part.contains(':'))
    {
        return Err(archive_error(
            "native archive entry path contains a platform prefix",
        ));
    }

    ManagedRelativePath::new(trimmed).map_err(|source| {
        archive_error("native archive entry path is not a managed path").with_source(source)
    })
}
fn checkpoint(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        Err(GrapheneError::new(
            ErrorCode::InstallCancelled,
            ErrorKind::Cancelled,
            "installation was cancelled during native extraction",
        ))
    } else {
        Ok(())
    }
}

fn archive_error(message: &'static str) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::InstallNativeExtractionFailed,
        ErrorKind::Install,
        message,
    )
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_names_are_rejected() {
        assert!(validate_entry_name("../evil.dll").is_err());
        assert!(validate_entry_name("/absolute.dll").is_err());
        assert!(validate_entry_name("C:/evil.dll").is_err());
        assert!(validate_entry_name("good/native.dll").is_ok());
    }

    #[test]
    fn bounded_archive_parser_property_inputs_do_not_panic() {
        let mut state = 0xa409_3822_u32;
        for length in 0..512usize {
            let mut bytes = Vec::with_capacity(length);
            for _ in 0..length {
                state = state.wrapping_mul(22_695_477).wrapping_add(1);
                bytes.push((state >> 16) as u8);
            }
            let _ = central_entries(&bytes);
        }

        for value in [
            "a",
            "../x",
            "/x",
            "C:/x",
            "a\\b",
            "a/b/c",
            "META-INF/MANIFEST.MF",
        ] {
            let _ = validate_entry_name(value);
        }
    }

    #[test]
    fn crc32_known_vector() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/native-jars")
            .join(name)
    }

    fn temporary_directory(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "graphene-native-archive-{label}-{}",
            graphene_core::ArtifactId::new()
        ));
        fs::create_dir(&path).expect("create test directory");
        path
    }

    #[test]
    fn frozen_deflated_native_fixture_extracts_without_meta_inf() {
        let root = temporary_directory("valid");
        extract_native_zip(
            &fixture("fixture-native.jar"),
            &root,
            &CancellationToken::new(),
        )
        .expect("extract fixture");
        assert_eq!(
            fs::read(root.join("native/fixture-native.bin")).expect("native bytes"),
            b"graphene-native-fixture\n"
        );
        assert!(!root.join("META-INF/MANIFEST.MF").exists());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn frozen_unsafe_native_archives_are_rejected() {
        for name in ["traversal.jar", "absolute.jar", "symlink.jar"] {
            let root = temporary_directory(name);
            let result = extract_native_zip(&fixture(name), &root, &CancellationToken::new());
            assert!(result.is_err(), "{name} unexpectedly extracted");
            fs::remove_dir_all(root).expect("cleanup");
        }
    }
}
