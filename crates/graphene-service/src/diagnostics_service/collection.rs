//! Instance-contained, bounded, deterministic diagnostic source collection.
//!
//! Collection never recursively sweeps the instance. It inspects a conservative allowlist of
//! metadata, logs, crash reports, and JVM fatal-error logs, rejecting symlinks and special files
//! and enforcing per-source and aggregate byte ceilings.

use graphene_core::{CancellationToken, ErrorCode, ErrorKind, GrapheneError, InstanceId, Result};
use graphene_diagnostics::{
    DiagnosticCompleteness, DiagnosticSourcePolicy, DiagnosticSourceSummary, EvidenceSourceKind,
    TextSource, decode_lossy,
};
use graphene_instance::ManagedRelativePath;
use graphene_storage::InstancePaths;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// Result of collecting bounded diagnostic text sources for one instance.
#[derive(Debug)]
pub(crate) struct CollectedSources {
    pub text_sources: Vec<TextSource>,
    pub summaries: Vec<DiagnosticSourceSummary>,
    pub completeness: DiagnosticCompleteness,
}

const LOG_FILES: &[&str] = &["logs/latest.log", "logs/debug.log"];
const CRASH_REPORT_SUFFIX: &str = ".txt";
const HS_ERR_PREFIX: &str = "hs_err_pid";
const HS_ERR_SUFFIX: &str = ".log";

fn collection_failed(message: impl Into<String>) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::DiagnosticCollectionFailed,
        ErrorKind::Diagnostics,
        message,
    )
}

fn source_unsafe(message: impl Into<String>) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::DiagnosticSourceUnsafe,
        ErrorKind::Diagnostics,
        message,
    )
}

fn relative_managed(root: &Path, path: &Path) -> Option<ManagedRelativePath> {
    let relative = path.strip_prefix(root).ok()?;
    let mut joined = String::new();

    for component in relative.components() {
        let piece = component.as_os_str().to_string_lossy();
        if piece.is_empty() {
            continue;
        }
        if !joined.is_empty() {
            joined.push('/');
        }
        joined.push_str(&piece);
    }

    ManagedRelativePath::new(joined).ok()
}

fn select_newest(directory: &Path, matches: impl Fn(&str) -> bool, limit: usize) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !matches(&name) {
            continue;
        }

        // `DirEntry::metadata` does not traverse symlinks, so crafted links are skipped.
        let Ok(metadata) = entry.metadata() else {
            continue;
        };

        if !metadata.is_file() {
            continue;
        }

        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        candidates.push((modified, entry.path()));
    }

    candidates.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    candidates
        .into_iter()
        .take(limit)
        .map(|(_, path)| path)
        .collect()
}

struct Collector<'a> {
    root: PathBuf,
    canonical_root: PathBuf,
    policy: &'a DiagnosticSourcePolicy,
    cancellation: &'a CancellationToken,
    bytes_inspected: u64,
    text_sources: Vec<TextSource>,
    summaries: Vec<DiagnosticSourceSummary>,
    completeness: DiagnosticCompleteness,
}

impl Collector<'_> {
    fn mark_partial(&mut self) {
        if self.completeness == DiagnosticCompleteness::Complete {
            self.completeness = DiagnosticCompleteness::Partial;
        }
    }

    fn collect_text(&mut self, path: &Path, source: EvidenceSourceKind) -> Result<()> {
        if self.cancellation.is_cancelled() {
            return Err(GrapheneError::new(
                ErrorCode::OperationCancelled,
                ErrorKind::Cancelled,
                "diagnostic collection was cancelled",
            ));
        }

        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(error) => {
                return Err(
                    collection_failed("failed inspecting diagnostic source").with_source(error)
                );
            }
        };

        if metadata.file_type().is_symlink() {
            return Err(source_unsafe("diagnostic source is a symbolic link"));
        }

        if !metadata.is_file() {
            self.summaries.push(
                DiagnosticSourceSummary::new(source, 0, false)
                    .with_note("source is not a regular file"),
            );
            self.mark_partial();
            return Ok(());
        }

        let parent = path.parent().unwrap_or(&self.root);
        let canonical_parent = fs::canonicalize(parent).map_err(|error| {
            collection_failed("failed resolving diagnostic source parent").with_source(error)
        })?;

        if !canonical_parent.starts_with(&self.canonical_root) {
            return Err(source_unsafe(
                "diagnostic source escapes the instance directory",
            ));
        }

        if self.bytes_inspected >= self.policy.max_total_bytes {
            self.mark_partial();
            return Ok(());
        }

        let allowance = self
            .policy
            .max_source_bytes
            .min(self.policy.max_total_bytes - self.bytes_inspected);
        let bytes = read_head(path, allowance)?;
        let truncated = metadata.len() > bytes.len() as u64;
        let text = decode_lossy(&bytes);

        self.bytes_inspected += bytes.len() as u64;

        let relative = relative_managed(&self.root, path);
        let mut text_source = TextSource::new(source, relative.clone(), text);

        if truncated {
            text_source = text_source.truncated();
            self.mark_partial();
        }

        self.text_sources.push(text_source);
        let mut summary = DiagnosticSourceSummary::new(source, bytes.len() as u64, truncated);
        if let Some(relative) = relative {
            summary = summary.with_path(relative);
        }

        self.summaries.push(summary);
        Ok(())
    }
}

fn read_head(path: &Path, max_bytes: u64) -> Result<Vec<u8>> {
    let file = fs::File::open(path).map_err(|error| {
        collection_failed("failed opening diagnostic source").with_source(error)
    })?;
    let mut buffer = Vec::new();

    file.take(max_bytes)
        .read_to_end(&mut buffer)
        .map_err(|error| {
            collection_failed("failed reading diagnostic source").with_source(error)
        })?;

    Ok(buffer)
}

fn metadata_summaries(
    paths: &InstancePaths,
    root: &Path,
    instance_id: InstanceId,
) -> Vec<DiagnosticSourceSummary> {
    let mut summaries = Vec::new();
    let candidates = [
        (
            EvidenceSourceKind::InstanceMetadata,
            paths.instance_descriptor_path(instance_id),
        ),
        (
            EvidenceSourceKind::InstallReceipt,
            paths.install_receipt_path(instance_id),
        ),
        (
            EvidenceSourceKind::DesiredStateLockfile,
            paths.lockfile_path(instance_id),
        ),
        (
            EvidenceSourceKind::InstanceConfig,
            paths.config_path(instance_id),
        ),
    ];

    for (kind, path) in candidates {
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };

        if !metadata.is_file() || metadata.file_type().is_symlink() {
            continue;
        }

        let mut summary = DiagnosticSourceSummary::new(kind, metadata.len(), false);
        if let Some(relative) = relative_managed(root, &path) {
            summary = summary.with_path(relative);
        }

        summaries.push(summary);
    }

    summaries
}

/// Collects bounded, instance-contained diagnostic text sources.
pub(crate) fn collect_sources(
    paths: &InstancePaths,
    instance_id: InstanceId,
    policy: &DiagnosticSourcePolicy,
    cancellation: &CancellationToken,
) -> Result<CollectedSources> {
    let root = paths.instance_root(instance_id);
    let canonical_root = fs::canonicalize(&root)
        .map_err(|error| collection_failed("failed resolving instance root").with_source(error))?;
    let minecraft = paths.minecraft_dir(instance_id);

    let mut collector = Collector {
        root: root.clone(),
        canonical_root,
        policy,
        cancellation,
        bytes_inspected: 0,
        text_sources: Vec::new(),
        summaries: metadata_summaries(paths, &root, instance_id),
        completeness: DiagnosticCompleteness::Complete,
    };

    if policy.collect_logs {
        for relative in LOG_FILES {
            collector.collect_text(&minecraft.join(relative), EvidenceSourceKind::LogFile)?;
        }
    }

    if policy.collect_crash_reports {
        for path in select_newest(
            &minecraft.join("crash-reports"),
            |name| name.ends_with(CRASH_REPORT_SUFFIX),
            policy.max_crash_reports,
        ) {
            collector.collect_text(&path, EvidenceSourceKind::CrashReport)?;
        }
    }

    if policy.collect_hs_err {
        for path in select_newest(
            &minecraft,
            |name| name.starts_with(HS_ERR_PREFIX) && name.ends_with(HS_ERR_SUFFIX),
            policy.max_hs_err_reports,
        ) {
            collector.collect_text(&path, EvidenceSourceKind::HsErrLog)?;
        }
    }

    Ok(CollectedSources {
        text_sources: collector.text_sources,
        summaries: collector.summaries,
        completeness: collector.completeness,
    })
}
