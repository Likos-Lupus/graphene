//! Provider-neutral content domain for Minecraft mods and remote catalogs.
//!
//! Owns content identities, normalized models, compatibility evaluation,
//! offline inventory scanning, and bounded dependency resolution.

pub mod compatibility;
pub mod dependency;
pub mod error;
pub mod id;
pub mod local;
pub mod model;
pub mod plan;
pub mod provider;

pub use id::{
    ContentEntryId, ContentFileRef, ContentProjectRef, ContentProviderId, ContentVersionRef,
};
pub use local::{
    ContentInventoryFingerprint, DuplicateModFinding, LocalContentFile, LocalContentInventory,
    LocalFileStatus, LocalModDependency, ModMetadataSource, NormalizedModDescriptor,
    compute_murmur2, scan_local_inventory, stream_file_hashes,
};
pub use model::{
    CompatibilityResult, ContentDependency, ContentFile, ContentKind, ContentProject,
    ContentSearchHit, ContentSearchPage, ContentSide, ContentVersion, DependencyRelation,
    DependencyTarget, EnvironmentSupport, FileRole, IncompatibilityReason, InstanceContentContext,
    ReleaseChannel, ReleaseChannelPolicy,
};
pub use plan::{
    CONTENT_PLAN_SCHEMA_VERSION, ContentActionRequest, ContentMutationPlan, ContentMutationRequest,
    MAX_PLAN_ACTIONS, MAX_PLAN_ENTRIES, MAX_PLAN_FILESYSTEM_ACTIONS, PlannedContentEntry,
    PlannedFilesystemAction, sanitize_mod_filename,
};
pub use provider::{
    ContentFileMatch, ContentProvider, ContentProviderCapabilities, ContentProviderFuture,
    ContentSearchQuery, ContentVersionFilter, ExactMatchStatus, FileMatchItem, FileMatchRequest,
};

#[cfg(test)]
mod tests;
