//! Normalized modpack domain models shared by every format adapter.

pub mod file;
pub mod format;
pub mod inspection;
pub mod metadata;
pub mod modpack;
pub mod runtime;
pub mod seed;
pub mod selection;
pub mod snapshot;

pub use file::{
    ContentHint, EmbeddedPackFile, FileSelection, ManagedSource, NormalizedPackFile,
    PendingProviderFile, ProviderFileRef,
};
pub use format::PackFormat;
pub use inspection::PackInspection;
pub use metadata::PackMetadata;
pub use modpack::{NormalizedModpack, PackDiagnostic, PackDiagnosticCode};
pub use runtime::{PackLoaderRequirement, PackRuntimeRequirement};
pub use seed::{NormalizedSeedEntry, SeedLayer};
pub use selection::{OptionalChoicePolicy as OptionalSelectionPolicy, PackOptionalChoice};
pub use snapshot::PackSourceSnapshot;

use serde::{Deserialize, Serialize};

/// Target side of a pack import. The current engine installs client instances only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PackTargetSide {
    Client,
}
