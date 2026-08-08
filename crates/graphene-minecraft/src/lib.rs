//! Provider-neutral Minecraft metadata and deterministic Vanilla resolution.
//!
//! This crate performs no network, filesystem, process, or UI work. Provider DTOs must be
//! normalized before entering these types. The crate root is a facade; domain behavior is owned by
//! cohesive internal modules.

mod argument;
mod error;
mod inheritance;
mod library;
mod maven;
mod metadata;
mod path;
mod resolution;
mod rules;
#[cfg(test)]
mod test_support;
mod version;

pub use argument::{Argument, tokenize_legacy_arguments};
pub use inheritance::{InheritanceTracker, MAX_INHERITANCE_DEPTH};
pub use library::{Library, ResolvedLibrary};
pub use maven::MavenCoordinate;
pub use metadata::{
    MinecraftJavaRequirement, MinecraftVersionMetadata, ResolvedArtifact, ResolvedAssetObject,
    ResolvedAssets, ResolvedLogging, merge_metadata,
};
pub use path::ManagedPath;
pub use resolution::{ResolvedMinecraft, resolve_minecraft};
pub use rules::{MinecraftArch, MinecraftOs, OsRule, Rule, RuleAction, RuleContext, rules_allow};
pub use version::{
    LatestVersions, MinecraftVersionId, MinecraftVersionType, VersionManifest, VersionSummary,
};
