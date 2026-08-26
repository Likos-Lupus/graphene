use crate::archive::PackPath;
use crate::error::PackError;
use serde::{Deserialize, Serialize};

/// Bounded, actionable non-fatal evidence attached to pack results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackDiagnostic {
    code: PackDiagnosticCode,
    message: String,
}

impl PackDiagnostic {
    pub fn new(code: PackDiagnosticCode, message: impl Into<String>) -> Result<Self, PackError> {
        let message = message.into();
        if message.chars().count() > 512 {
            return Err(PackError::manifest("diagnostic message exceeds its bound"));
        }

        Ok(Self { code, message })
    }

    #[must_use]
    pub const fn code(&self) -> PackDiagnosticCode {
        self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Stable diagnostic categories hosts can branch on without string parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PackDiagnosticCode {
    ServerOnlyDataSkipped,
    OptionalFileNotSelected,
    SourceHasNoAuthenticityProof,
    ManagedArtifactCacheOnly,
    EmbeddedModHasNoProviderIdentity,
    ForeignLauncherMetadataIgnored,
    GenericRuntimeSuppliedByCaller,
    ExportEmbeddingDecisionRequired,
    SeedSelectionMayContainPrivateData,
    PackOriginIsHistoricalProvenance,
}

/// The normalized Graphene-owned pack model every supported format converges to.
#[derive(Debug, Clone)]
pub struct NormalizedModpack {
    format: crate::model::format::PackFormat,
    metadata: super::metadata::PackMetadata,
    runtime: super::runtime::PackRuntimeRequirement,
    managed_files: Vec<super::file::NormalizedPackFile>,
    pending_provider_files: Vec<super::file::PendingProviderFile>,
    embedded_files: Vec<super::file::EmbeddedPackFile>,
    seed_entries: Vec<super::seed::NormalizedSeedEntry>,
    optional_choices: Vec<super::selection::PackOptionalChoice>,
    diagnostics: Vec<PackDiagnostic>,
}

impl NormalizedModpack {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        format: crate::model::format::PackFormat,
        metadata: super::metadata::PackMetadata,
        runtime: super::runtime::PackRuntimeRequirement,
        managed_files: Vec<super::file::NormalizedPackFile>,
        pending_provider_files: Vec<super::file::PendingProviderFile>,
        embedded_files: Vec<super::file::EmbeddedPackFile>,
        seed_entries: Vec<super::seed::NormalizedSeedEntry>,
        optional_choices: Vec<super::selection::PackOptionalChoice>,
        diagnostics: Vec<PackDiagnostic>,
    ) -> Result<Self, PackError> {
        if managed_files
            .len()
            .saturating_add(pending_provider_files.len())
            .saturating_add(embedded_files.len())
            > crate::archive::limits::MAX_MANAGED_FILES
        {
            return Err(PackError::new(
                graphene_core::ErrorCode::PackSourceTooLarge,
                "managed file count exceeds its bound",
            ));
        }
        if seed_entries.len() > crate::archive::limits::MAX_SEED_ENTRIES {
            return Err(PackError::new(
                graphene_core::ErrorCode::PackSourceTooLarge,
                "seed entry count exceeds its bound",
            ));
        }
        let mut total_declared = 0_u64;
        let mut seen: std::collections::BTreeMap<String, &str> = std::collections::BTreeMap::new();
        for file in &managed_files {
            total_declared += file.size();
            let key = file.destination().collision_key();
            if seen.contains_key(&key) {
                return Err(PackError::path(format!(
                    "managed destination collides case-insensitively: {}",
                    seen[&key]
                )));
            }
            seen.insert(key, file.destination().as_str());
        }
        let mut provider_ids = std::collections::BTreeSet::new();
        for file in &pending_provider_files {
            if !provider_ids.insert(file.stable_id()) {
                return Err(PackError::manifest(
                    "pack repeats an exact provider file declaration",
                ));
            }
        }
        for file in &embedded_files {
            total_declared += file.size();
            let key = file.destination().collision_key();
            if seen.contains_key(&key) {
                return Err(PackError::path(
                    "embedded file collides with a managed destination",
                ));
            }
            seen.insert(key, file.destination().as_str());
        }
        if total_declared > crate::archive::limits::MAX_DECLARED_MANAGED_BYTES {
            return Err(PackError::new(
                graphene_core::ErrorCode::PackSourceTooLarge,
                "declared managed bytes exceed their bound",
            ));
        }
        let mut seed_total = 0_u64;
        // Later layers may replace earlier ones only where a format documents precedence;
        // equal-layer duplicates are always fatal.
        let mut layered: std::collections::BTreeMap<String, u8> = std::collections::BTreeMap::new();
        for entry in &seed_entries {
            seed_total += entry.size();
            let key = entry.destination().collision_key();
            match layered.get(&key) {
                Some(existing_layer) if *existing_layer >= entry.layer().value() => {
                    return Err(PackError::path(format!(
                        "seed destination repeats without later-layer precedence: {}",
                        entry.destination().as_str()
                    )));
                }
                Some(_) => {
                    layered.insert(key.clone(), entry.layer().value());
                }
                None => {
                    layered.insert(key.clone(), entry.layer().value());
                }
            }
            // A seed entry must never replace a managed downloaded file's destination.
            if seen.contains_key(&key) {
                return Err(PackError::path(
                    "seed entry collides with a managed file destination",
                ));
            }
        }
        if seed_total.saturating_add(total_declared)
            > crate::archive::limits::MAX_TOTAL_EXPANSION_BYTES
        {
            return Err(PackError::new(
                graphene_core::ErrorCode::PackSourceTooLarge,
                "total pack payload exceeds the expansion bound",
            ));
        }

        Ok(Self {
            format,
            metadata,
            runtime,
            managed_files,
            pending_provider_files,
            embedded_files,
            seed_entries,
            optional_choices,
            diagnostics,
        })
    }

    /// Deterministic ordering pass applied by adapters after normalization.
    pub fn sort_for_stability(&mut self) {
        self.managed_files
            .sort_by(|a, b| a.destination().as_str().cmp(b.destination().as_str()));
        self.pending_provider_files
            .sort_by_key(super::file::PendingProviderFile::stable_id);
        self.embedded_files
            .sort_by(|a, b| a.destination().as_str().cmp(b.destination().as_str()));
        self.seed_entries.sort_by(|a, b| {
            a.layer()
                .cmp(&b.layer())
                .then_with(|| a.destination().as_str().cmp(b.destination().as_str()))
        });
        self.optional_choices.sort_by(|a, b| a.id().cmp(b.id()));
    }

    #[must_use]
    pub const fn format(&self) -> crate::model::format::PackFormat {
        self.format
    }

    #[must_use]
    pub const fn metadata(&self) -> &super::metadata::PackMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn runtime(&self) -> &super::runtime::PackRuntimeRequirement {
        &self.runtime
    }

    #[must_use]
    pub fn managed_files(&self) -> &[super::file::NormalizedPackFile] {
        &self.managed_files
    }

    #[must_use]
    pub fn pending_provider_files(&self) -> &[super::file::PendingProviderFile] {
        &self.pending_provider_files
    }

    #[must_use]
    pub fn embedded_files(&self) -> &[super::file::EmbeddedPackFile] {
        &self.embedded_files
    }

    #[must_use]
    pub fn seed_entries(&self) -> &[super::seed::NormalizedSeedEntry] {
        &self.seed_entries
    }

    #[must_use]
    pub fn optional_choices(&self) -> &[super::selection::PackOptionalChoice] {
        &self.optional_choices
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[PackDiagnostic] {
        &self.diagnostics
    }

    /// Counts of embedded mod payloads (used by inspection summaries).
    #[must_use]
    pub fn embedded_mod_count(&self) -> usize {
        let embedded = self
            .embedded_files
            .iter()
            .filter(|entry| {
                entry.content_hint() == Some(super::file::ContentHint::Mod)
                    || is_mod_destination(entry.destination())
            })
            .count();
        let seed_mods = self
            .seed_entries
            .iter()
            .filter(|entry| is_mod_destination(entry.destination()))
            .count();
        embedded + seed_mods
    }
}

/// Returns whether a normalized destination lands in the enabled/disabled mods area.
#[must_use]
pub fn is_mod_destination(destination: &PackPath) -> bool {
    let components: Vec<&str> = destination.components().collect();
    components.len() == 2
        && components[0] == "mods"
        && (components[1].to_ascii_lowercase().ends_with(".jar")
            || components[1]
                .to_ascii_lowercase()
                .ends_with(".jar.disabled"))
}
