use crate::ManagedRelativePath;
use serde::{Deserialize, Serialize};

/// Specific, closed domain action required to restore instance integrity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "action_kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RepairAction {
    /// Acquires or verifies a required artifact in the content-addressed cache.
    AcquireArtifact {
        logical_key: String,
        destination: ManagedRelativePath,
    },
    /// Re-materializes a shared immutable library or asset in `shared/`.
    RestoreSharedMaterialization { destination: ManagedRelativePath },
    /// Restores a managed instance file under `.minecraft/` or `.graphene/`.
    RestoreInstanceMaterialization { destination: ManagedRelativePath },
    /// Cleans and re-extracts native libraries for an installed version.
    RebuildNativeState { destination: ManagedRelativePath },
    /// Re-runs Java-based loader preparation processors to produce generated outputs.
    RunPreparation { component_uid: String },
    /// Replaces a damaged or missing generated loader output.
    RestoreGeneratedOutput { destination: ManagedRelativePath },
    /// Rewrites the install receipt to align with desired lockfile state.
    RewriteInstallReceipt,
    /// Finalizes desired-state lockfile migration for a legacy instance.
    FinalizeLockfileMigration,
}
