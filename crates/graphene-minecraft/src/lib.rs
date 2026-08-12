//! Provider-neutral Minecraft metadata and deterministic Vanilla resolution.
//!
//! This crate performs no network, filesystem, process, or UI work. Provider DTOs must be
//! normalized before entering these types. The crate root is a facade; domain behavior is owned by
//! cohesive internal modules.

mod argument;
mod component;
mod error;
mod inheritance;
mod library;
mod loader;
mod maven;
mod metadata;
mod patch;
mod path;
mod resolution;
mod rules;
#[cfg(test)]
mod test_support;
mod version;

pub use argument::{Argument, tokenize_legacy_arguments};
pub use component::{
    ComponentConflict, ComponentDescriptor, ComponentGraph, ComponentKind, ComponentProvenance,
    ComponentRequest, ComponentRequirement, ComponentUid, ComponentVersion, MAX_COMPONENT_EDGES,
    MAX_COMPONENT_NODES, ResolvedComponent,
};
pub use inheritance::{InheritanceTracker, MAX_INHERITANCE_DEPTH};
pub use library::{Library, ResolvedLibrary};
pub use loader::preparation::{
    ComponentPreparationRecipe, EmbeddedInstallerInput, GeneratedOutput, GeneratedOutputScope,
    PreparationArgument, PreparationArgumentPart, PreparationDataValue, PreparationPlaceholder,
    ProcessorSideCondition, ProcessorStep,
};
pub use loader::{
    LoaderKind, LoaderProviderCapabilities, LoaderSelection, LoaderSupport, LoaderVersion,
    LoaderVersionSelector, LoaderVersionSummary, ResolvedLoader, loader_support_diagnostic,
};
pub use maven::MavenCoordinate;
pub use metadata::{
    MinecraftJavaRequirement, MinecraftVersionMetadata, ResolvedArtifact, ResolvedAssetObject,
    ResolvedAssets, ResolvedLogging, merge_metadata,
};
pub use patch::{MinecraftVersionPatch, compose_minecraft};
pub use path::ManagedPath;
pub use resolution::{ResolvedMinecraft, resolve_minecraft};
pub use rules::{MinecraftArch, MinecraftOs, OsRule, Rule, RuleAction, RuleContext, rules_allow};
pub use version::{
    LatestVersions, MinecraftVersionId, MinecraftVersionType, VersionManifest, VersionSummary,
};
