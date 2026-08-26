use super::{checkpoint, spawn_blocking_install};
use crate::{
    error::{cancelled_error, install_error},
    plan::{InstallPlan, SeedArchiveLayer},
};
use graphene_core::{ErrorCode, OperationController, Progress, Result};
use graphene_platform::ManagedRelativePath;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Entry-count bound applied when reopening a cached seed archive.
const SEED_ARCHIVE_MAX_ENTRIES: usize = 100_000;

/// Applies every planned seed archive layer into the isolated staging tree.
///
/// Layers are grouped per cached archive so each snapshot is opened once; entries stream through a
/// hashing writer so size and SHA-256 are verified against the plan after every write.
pub(super) async fn apply_seed_layers(
    plan: &InstallPlan,
    acquired_paths: &HashMap<graphene_core::ArtifactId, PathBuf>,
    staging_root: &Path,
    operation: &OperationController,
) -> Result<()> {
    if plan.seed_archive_layers.is_empty() {
        return Ok(());
    }

    operation.set_stage("apply-seed")?;
    let total = plan.seed_archive_layers.len() as u64;
    operation.set_progress(Progress::Items {
        completed: 0,
        total: Some(total),
    })?;

    let mut layers_by_archive: HashMap<graphene_core::ArtifactId, Vec<SeedArchiveLayer>> =
        HashMap::new();
    for layer in &plan.seed_archive_layers {
        layers_by_archive
            .entry(layer.archive_artifact_id)
            .or_default()
            .push(layer.clone());
    }
    let mut ordered_archives: Vec<graphene_core::ArtifactId> =
        layers_by_archive.keys().copied().collect();
    ordered_archives.sort_by_key(|id| id.to_string());

    let mut completed = 0_u64;
    for archive_id in ordered_archives {
        checkpoint(operation)?;
        let archive_path = acquired_paths.get(&archive_id).ok_or_else(|| {
            install_error(
                ErrorCode::InstallPlanInvalid,
                "seed archive artifact was not acquired",
            )
        })?;
        let path = archive_path.clone();
        let root = staging_root.to_path_buf();
        let layers = layers_by_archive.remove(&archive_id).unwrap_or_default();
        let layer_count = layers.len();
        let cancellation = operation.handle().cancellation_token();
        spawn_blocking_install(move || extract_seed_archive(&path, &layers, &root, &cancellation))
            .await?;
        completed += layer_count as u64;
        operation.set_progress(Progress::Items {
            completed,
            total: Some(total),
        })?;
    }

    Ok(())
}

fn extract_seed_archive(
    archive_path: &Path,
    layers: &[SeedArchiveLayer],
    staging_root: &Path,
    cancellation: &graphene_core::CancellationToken,
) -> Result<()> {
    if layers.is_empty() {
        return Ok(());
    }
    let file = fs::File::open(archive_path).map_err(|source| {
        install_error(
            ErrorCode::ArtifactSourceUnavailable,
            "seed archive snapshot is unreadable",
        )
        .with_source(source)
    })?;
    let mut archive = graphene_core::archive::ArchiveFile::open(
        file,
        SEED_ARCHIVE_MAX_ENTRIES,
        crate::plan::MAX_SEED_ENTRY_NAME_BYTES,
        cancellation,
    )
    .map_err(|source| {
        install_error(ErrorCode::PackArchiveInvalid, "seed archive failed to open")
            .with_source(source)
    })?;

    for layer in layers {
        if cancellation.is_cancelled() {
            return Err(cancelled_error());
        }
        extract_one(&mut archive, layer, staging_root, cancellation)?;
    }

    Ok(())
}

fn extract_one<R: std::io::Read + std::io::Seek>(
    archive: &mut graphene_core::archive::ArchiveFile<R>,
    layer: &SeedArchiveLayer,
    staging_root: &Path,
    cancellation: &graphene_core::CancellationToken,
) -> Result<()> {
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.name == layer.archive_entry)
        .cloned()
        .ok_or_else(|| {
            install_error(
                ErrorCode::PackArchiveInvalid,
                "planned seed archive entry is missing from the cached snapshot",
            )
        })?;
    if !entry.is_regular {
        return Err(install_error(
            ErrorCode::PackArchiveInvalid,
            "planned seed archive entry is not a regular file",
        ));
    }

    let relative = ManagedRelativePath::new(layer.destination.as_str()).map_err(|source| {
        install_error(ErrorCode::InstallPlanInvalid, "seed destination is invalid")
            .with_source(source)
    })?;
    let destination = relative.under(staging_root);
    let parent = destination.parent().ok_or_else(|| {
        install_error(
            ErrorCode::InstallStageFailed,
            "seed destination has no parent",
        )
    })?;
    fs::create_dir_all(parent).map_err(|source| {
        install_error(
            ErrorCode::InstallStageFailed,
            "failed to create seed parent directory",
        )
        .with_source(source)
    })?;

    let output = fs::File::create(&destination).map_err(|source| {
        install_error(
            ErrorCode::InstallStageFailed,
            "failed to create seed target file",
        )
        .with_source(source)
    })?;
    let mut writer = HashingWriter::new(output);
    let written = archive
        .stream_entry_to(&entry, layer.expected_size, &mut writer, cancellation)
        .map_err(|source| {
            install_error(ErrorCode::PackArchiveInvalid, "failed to stream seed entry")
                .with_source(source)
        })?;
    let observed = writer.finish_and_digest().map_err(|source| {
        install_error(
            ErrorCode::InstallStageFailed,
            "failed to flush seed target file",
        )
        .with_source(source)
    })?;
    if written != layer.expected_size || observed != *layer.expected_sha256.as_bytes() {
        return Err(install_error(
            ErrorCode::HashMismatch,
            "applied seed payload does not match its planned identity",
        ));
    }

    Ok(())
}

struct HashingWriter {
    inner: fs::File,
    hasher: Sha256,
}

impl HashingWriter {
    fn new(inner: fs::File) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
        }
    }

    fn finish_and_digest(mut self) -> std::io::Result<[u8; 32]> {
        self.inner.flush()?;
        self.inner.sync_all()?;
        Ok(self.hasher.finalize().into())
    }
}

impl Write for HashingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.hasher.update(buf);
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
