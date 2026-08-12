mod managed;

use graphene_core::{
    CancellationToken, ErrorCode, ErrorKind, GrapheneError, Result,
    archive::{ArchiveCodecError, central_entries, crc32, extract_entry},
};
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
    let entries =
        central_entries(&bytes, MAX_ENTRIES, MAX_NAME_BYTES).map_err(native_codec_error)?;
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

        let data = extract_entry(&bytes, &entry, MAX_ENTRY_BYTES, cancellation)
            .map_err(native_codec_error)?;
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

pub fn extract_managed_zip(
    archive: &Path,
    destination: &Path,
    cancellation: &CancellationToken,
) -> Result<()> {
    extract_native_zip(archive, destination, cancellation)
}

pub use managed::extract_tar_gz as extract_managed_tar_gz;

pub(crate) fn extract_selected_entry(
    archive: &Path,
    entry_name: &str,
    destination: &Path,
    maximum_size: u64,
    cancellation: &CancellationToken,
) -> Result<()> {
    let metadata = fs::symlink_metadata(archive).map_err(|source| {
        loader_archive_error("failed to inspect loader installer").with_source(source)
    })?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_ARCHIVE_BYTES
    {
        return Err(loader_archive_error(
            "loader installer is not a bounded ordinary file",
        ));
    }

    validate_entry_name(entry_name)
        .map_err(|_| loader_archive_error("loader installer entry path is unsafe"))?;

    let bytes = fs::read(archive).map_err(|source| {
        loader_archive_error("failed to read loader installer").with_source(source)
    })?;
    let entries =
        central_entries(&bytes, MAX_ENTRIES, MAX_NAME_BYTES).map_err(loader_codec_error)?;

    if entries.len() > MAX_ENTRIES {
        return Err(loader_archive_error(
            "loader installer contains too many entries",
        ));
    }

    let mut selected = None;
    for entry in entries {
        checkpoint(cancellation)?;
        if entry.name != entry_name {
            continue;
        }

        if selected.is_some() {
            return Err(loader_archive_error(
                "loader installer contains a duplicate selected entry",
            ));
        }

        if entry.is_directory || entry.is_symlink || !entry.is_regular {
            return Err(loader_archive_error(
                "loader installer selected entry has an unsafe type",
            ));
        }

        if entry.uncompressed_size as u64 > maximum_size
            || entry.uncompressed_size > MAX_ENTRY_BYTES
            || (entry.uncompressed_size > 0
                && (entry.compressed_size == 0
                    || entry.uncompressed_size
                        > entry.compressed_size.saturating_mul(MAX_EXPANSION_RATIO)))
        {
            return Err(loader_archive_error(
                "loader installer selected entry exceeds resource bounds",
            ));
        }

        selected = Some(entry);
    }

    let entry = selected
        .ok_or_else(|| loader_archive_error("loader installer selected entry is missing"))?;
    let data = extract_entry(
        &bytes,
        &entry,
        maximum_size.min(MAX_ENTRY_BYTES as u64) as usize,
        cancellation,
    )
    .map_err(loader_codec_error)?;

    if crc32(&data) != entry.crc32 {
        return Err(loader_archive_error(
            "loader installer selected entry CRC does not match",
        ));
    }

    if destination.file_name().is_none() {
        return Err(loader_archive_error(
            "loader installer extraction destination is invalid",
        ));
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|source| {
            loader_archive_error("failed to create loader installer input directory")
                .with_source(source)
        })?;
    }

    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(|source| {
            loader_archive_error("failed to create loader installer input").with_source(source)
        })?;

    file.write_all(&data).map_err(|source| {
        loader_archive_error("failed to write loader installer input").with_source(source)
    })?;

    file.sync_all().map_err(|source| {
        loader_archive_error("failed to sync loader installer input").with_source(source)
    })?;

    Ok(())
}

pub(crate) fn processor_main_class(
    archive: &Path,
    cancellation: &CancellationToken,
) -> Result<String> {
    const MANIFEST: &str = "META-INF/MANIFEST.MF";
    const MAX_MANIFEST_BYTES: usize = 64 * 1024;

    let metadata = fs::symlink_metadata(archive).map_err(|source| {
        loader_archive_error("failed to inspect loader processor JAR").with_source(source)
    })?;

    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_ARCHIVE_BYTES
    {
        return Err(loader_archive_error(
            "loader processor JAR is not a bounded ordinary file",
        ));
    }

    checkpoint(cancellation)?;

    let bytes = fs::read(archive).map_err(|source| {
        loader_archive_error("failed to read loader processor JAR").with_source(source)
    })?;
    let entries =
        central_entries(&bytes, MAX_ENTRIES, MAX_NAME_BYTES).map_err(loader_codec_error)?;

    let mut selected = None;
    for entry in entries {
        if entry.name != MANIFEST {
            continue;
        }

        if selected.is_some() || entry.is_directory || entry.is_symlink || !entry.is_regular {
            return Err(loader_archive_error(
                "loader processor manifest entry is ambiguous or unsafe",
            ));
        }

        if entry.uncompressed_size > MAX_MANIFEST_BYTES {
            return Err(loader_archive_error(
                "loader processor manifest exceeds its size bound",
            ));
        }

        selected = Some(entry);
    }

    let entry = selected.ok_or_else(|| {
        GrapheneError::new(
            ErrorCode::LoaderProcessorUnsupported,
            ErrorKind::Install,
            "loader processor JAR has no manifest",
        )
    })?;
    let manifest = extract_entry(&bytes, &entry, MAX_MANIFEST_BYTES, cancellation)
        .map_err(loader_codec_error)?;

    if crc32(&manifest) != entry.crc32 {
        return Err(loader_archive_error(
            "loader processor manifest CRC does not match",
        ));
    }

    parse_manifest_main_class(&manifest)
}

fn parse_manifest_main_class(bytes: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(bytes).map_err(|source| {
        GrapheneError::new(
            ErrorCode::LoaderProcessorUnsupported,
            ErrorKind::Install,
            "loader processor manifest is not UTF-8",
        )
        .with_source(source)
    })?;

    let mut logical = Vec::<String>::new();
    for raw in text.replace("\r\n", "\n").split('\n') {
        if let Some(continuation) = raw.strip_prefix(' ') {
            let previous = logical.last_mut().ok_or_else(|| {
                GrapheneError::new(
                    ErrorCode::LoaderProcessorUnsupported,
                    ErrorKind::Install,
                    "loader processor manifest starts with an invalid continuation",
                )
            })?;
            previous.push_str(continuation);
        } else if !raw.is_empty() {
            logical.push(raw.to_owned());
        }
    }

    let value = logical
        .iter()
        .find_map(|line| line.strip_prefix("Main-Class:"))
        .map(str::trim)
        .ok_or_else(|| {
            GrapheneError::new(
                ErrorCode::LoaderProcessorUnsupported,
                ErrorKind::Install,
                "loader processor manifest has no Main-Class entry",
            )
        })?;

    if value.is_empty()
        || value.len() > 512
        || value.contains('\0')
        || value.chars().any(char::is_whitespace)
    {
        return Err(GrapheneError::new(
            ErrorCode::LoaderProcessorUnsupported,
            ErrorKind::Install,
            "loader processor manifest Main-Class is invalid",
        ));
    }

    Ok(value.to_owned())
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

fn native_codec_error(source: ArchiveCodecError) -> GrapheneError {
    if source.is_cancelled() {
        GrapheneError::new(
            ErrorCode::InstallCancelled,
            ErrorKind::Cancelled,
            "installation was cancelled during native extraction",
        )
    } else {
        archive_error("native archive structure or compression is invalid").with_source(source)
    }
}

fn loader_codec_error(source: ArchiveCodecError) -> GrapheneError {
    if source.is_cancelled() {
        GrapheneError::new(
            ErrorCode::LoaderProcessorCancelled,
            ErrorKind::Cancelled,
            "loader archive operation was cancelled",
        )
    } else {
        loader_archive_error("loader archive structure or compression is invalid")
            .with_source(source)
    }
}

fn loader_archive_error(message: &'static str) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::LoaderInstallerInvalid,
        ErrorKind::Install,
        message,
    )
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
    fn manifest_main_class_parser_is_bounded_and_handles_continuations() {
        assert_eq!(
            parse_manifest_main_class(
                b"Manifest-Version: 1.0\r\nMain-Class: com.example.\r\n Tool\r\n"
            )
            .expect("main class"),
            "com.example.Tool"
        );
        assert!(parse_manifest_main_class(b"Manifest-Version: 1.0\n").is_err());
        assert!(parse_manifest_main_class(b"Main-Class: bad class\n").is_err());
    }

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
            let _ = central_entries(&bytes, MAX_ENTRIES, MAX_NAME_BYTES);
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

    fn managed_fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/managed-java")
            .join(name)
    }

    #[test]
    fn frozen_managed_zip_and_tar_gz_fixtures_extract() {
        for (name, tar_gz) in [("valid.zip", false), ("valid.tar.gz", true)] {
            let root = temporary_directory(name);
            let result = if tar_gz {
                extract_managed_tar_gz(&managed_fixture(name), &root, &CancellationToken::new())
            } else {
                extract_managed_zip(&managed_fixture(name), &root, &CancellationToken::new())
            };
            result.expect("valid managed archive");
            assert_eq!(
                fs::read(root.join("runtime/bin/java")).expect("java fixture"),
                b"fixture-java\n"
            );
            fs::remove_dir_all(root).expect("cleanup");
        }
    }

    #[test]
    fn frozen_hostile_managed_archives_never_escape_or_commit() {
        let cases = [
            ("traversal.zip", false),
            ("absolute.zip", false),
            ("backslash.zip", false),
            ("symlink.zip", false),
            ("oversized-entry.zip", false),
            ("bomb-policy.zip", false),
            ("corrupt.zip", false),
            ("traversal.tar.gz", true),
            ("absolute.tar.gz", true),
            ("symlink.tar.gz", true),
            ("hardlink.tar.gz", true),
            ("oversized-entry.tar.gz", true),
            ("bomb-policy.tar.gz", true),
            ("corrupt.tar.gz", true),
        ];
        for (name, tar_gz) in cases {
            let outer = temporary_directory(&format!("managed-hostile-{name}"));
            let root = outer.join("extract");
            fs::create_dir(&root).expect("create extraction root");
            let result = if tar_gz {
                extract_managed_tar_gz(&managed_fixture(name), &root, &CancellationToken::new())
            } else {
                extract_managed_zip(&managed_fixture(name), &root, &CancellationToken::new())
            };
            assert!(result.is_err(), "{name} unexpectedly extracted");
            assert!(!outer.join("escape.txt").exists());
            fs::remove_dir_all(outer).expect("cleanup");
        }
    }
}
