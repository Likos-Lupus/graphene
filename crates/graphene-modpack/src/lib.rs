//! Provider-neutral modpack bounded context.
//!
//! This crate owns Graphene's stable modpack vocabulary: normalized pack models, deterministic
//! format detection, bounded archive path policy, and pure plan/validation models. It performs no
//! network access, owns no transport, and never exposes foreign manifest DTOs.

pub mod archive;
pub mod error;
pub mod format;
pub mod model;

pub use archive::{PackArchiveIndex, PackPath};
pub use error::PackError;
pub use format::{DetectedFormatRoots, FormatDetection, detect_pack_format};
pub use model::{
    ContentHint, EmbeddedPackFile, FileSelection, ManagedSource, NormalizedModpack,
    NormalizedPackFile, NormalizedSeedEntry, OptionalSelectionPolicy, PackDiagnostic,
    PackDiagnosticCode, PackFormat, PackInspection, PackMetadata, PackOptionalChoice,
    PackRuntimeRequirement, PackSourceSnapshot, PackTargetSide, PendingProviderFile,
    ProviderFileRef, SeedLayer,
};

/// Crate-local result alias over the pack error contract.
pub type PackResult<T> = Result<T, PackError>;
