use crate::model::format::PackFormat;
use crate::model::metadata::PackMetadata;
use crate::model::modpack::PackDiagnostic;
use crate::model::runtime::PackRuntimeRequirement;
use crate::model::selection::PackOptionalChoice;
use crate::model::snapshot::PackSourceSnapshot;
use serde::{Deserialize, Serialize};

/// Safe, bounded inspection result presented to hosts before any planning.
///
/// It exposes only normalized values: no archive handles, cache paths, provider DTOs, or raw URLs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackInspection {
    format: PackFormat,
    snapshot: PackSourceSnapshot,
    metadata: PackMetadata,
    runtime: Option<PackRuntimeRequirement>,
    missing_runtime_reason: Option<String>,
    required_file_count: usize,
    optional_choices: Vec<PackOptionalChoice>,
    excluded_for_client_count: usize,
    seed_entry_count: usize,
    seed_bytes: u64,
    embedded_mod_count: usize,
    expected_download_bytes: u64,
    diagnostics: Vec<PackDiagnostic>,
    requires_provider_resolution: bool,
}

impl PackInspection {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        format: PackFormat,
        snapshot: PackSourceSnapshot,
        metadata: PackMetadata,
        runtime: Option<PackRuntimeRequirement>,
        missing_runtime_reason: Option<String>,
        required_file_count: usize,
        optional_choices: Vec<PackOptionalChoice>,
        excluded_for_client_count: usize,
        seed_entry_count: usize,
        seed_bytes: u64,
        embedded_mod_count: usize,
        expected_download_bytes: u64,
        diagnostics: Vec<PackDiagnostic>,
        requires_provider_resolution: bool,
    ) -> Self {
        Self {
            format,
            snapshot,
            metadata,
            runtime,
            missing_runtime_reason,
            required_file_count,
            optional_choices,
            excluded_for_client_count,
            seed_entry_count,
            seed_bytes,
            embedded_mod_count,
            expected_download_bytes,
            diagnostics,
            requires_provider_resolution,
        }
    }

    /// Builds the summary from a normalized pack model plus its pinned source snapshot.
    #[must_use]
    pub fn from_normalized(
        normalized: &crate::model::modpack::NormalizedModpack,
        snapshot: PackSourceSnapshot,
    ) -> Self {
        let required = normalized
            .managed_files()
            .iter()
            .filter(|file| matches!(file.selection(), crate::model::FileSelection::Required))
            .count();
        let pending_required = normalized
            .pending_provider_files()
            .iter()
            .filter(|file| matches!(file.selection(), crate::model::FileSelection::Required))
            .count();
        let excluded = normalized
            .managed_files()
            .iter()
            .filter(|file| {
                matches!(
                    file.selection(),
                    crate::model::FileSelection::ExcludedForClient
                )
            })
            .count();
        let download_bytes = normalized
            .managed_files()
            .iter()
            .filter(|file| matches!(file.selection(), crate::model::FileSelection::Required))
            .map(|file| file.size())
            .sum();
        let seed_bytes = normalized.seed_entries().iter().map(|e| e.size()).sum();
        let requires_provider = normalized
            .managed_files()
            .iter()
            .any(|file| file.provider_ref().is_some());

        Self {
            format: normalized.format(),
            snapshot,
            metadata: normalized.metadata().clone(),
            runtime: Some(normalized.runtime().clone()),
            missing_runtime_reason: None,
            required_file_count: required + pending_required,
            optional_choices: normalized.optional_choices().to_vec(),
            excluded_for_client_count: excluded,
            seed_entry_count: normalized.seed_entries().len(),
            seed_bytes,
            embedded_mod_count: normalized.embedded_mod_count(),
            expected_download_bytes: download_bytes,
            diagnostics: normalized.diagnostics().to_vec(),
            requires_provider_resolution: requires_provider
                || !normalized.pending_provider_files().is_empty(),
        }
    }

    #[must_use]
    pub const fn format(&self) -> PackFormat {
        self.format
    }

    #[must_use]
    pub const fn snapshot(&self) -> &PackSourceSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub const fn metadata(&self) -> &PackMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn runtime(&self) -> Option<&PackRuntimeRequirement> {
        self.runtime.as_ref()
    }

    #[must_use]
    pub fn missing_runtime_reason(&self) -> Option<&str> {
        self.missing_runtime_reason.as_deref()
    }

    #[must_use]
    pub const fn required_file_count(&self) -> usize {
        self.required_file_count
    }

    #[must_use]
    pub fn optional_choices(&self) -> &[PackOptionalChoice] {
        &self.optional_choices
    }

    #[must_use]
    pub const fn excluded_for_client_count(&self) -> usize {
        self.excluded_for_client_count
    }

    #[must_use]
    pub const fn seed_entry_count(&self) -> usize {
        self.seed_entry_count
    }

    #[must_use]
    pub const fn seed_bytes(&self) -> u64 {
        self.seed_bytes
    }

    #[must_use]
    pub const fn embedded_mod_count(&self) -> usize {
        self.embedded_mod_count
    }

    #[must_use]
    pub const fn expected_download_bytes(&self) -> u64 {
        self.expected_download_bytes
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[PackDiagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub const fn requires_provider_resolution(&self) -> bool {
        self.requires_provider_resolution
    }
}
