//! Create-only instance identity and durable, provider-neutral installation metadata.
//!
//! The crate root is a facade. Schema ownership and validation are split by instance concept;
//! filesystem mutation, Minecraft resolution, provider parsing, and launch behavior remain outside
//! this bounded context.

mod clone;
mod committed;
mod config;
mod delete;
mod descriptor;
mod error;
mod inventory;
mod lockfile;
mod path;
mod receipt;
mod repair;
mod spec;
mod verification;

pub use clone::{CloneMode, CloneRequest};
pub use committed::CommittedInstance;
pub use config::{
    EffectiveInstanceConfig, GLOBAL_CONFIG_SCHEMA_VERSION, GlobalInstanceConfig,
    INSTANCE_CONFIG_SCHEMA_VERSION, InstanceConfig, InstanceConfigPatch, InstanceResolution,
    MAX_CONFIG_ARGUMENT_BYTES, MAX_CONFIG_ARGUMENTS, MAX_CONFIG_ENV_ENTRIES, MemoryPolicy,
    SettingUpdate, apply_patch_to_global, apply_patch_to_instance, resolve_effective_config,
};
pub use delete::DeleteOptions;
pub use descriptor::{INSTANCE_SCHEMA_VERSION, InstanceDescriptor};
pub use inventory::{InstanceInventoryEntry, InstanceStatus};
pub use lockfile::{
    InstanceLockfile, InstanceStateFingerprint, LOCKFILE_SCHEMA_VERSION, LockedArtifact,
    LockedContentDependency, LockedContentEntry, LockedGeneratedOutput, LockedMaterializationScope,
    LockedNativeExtraction, MAX_LOCKED_ARTIFACTS, MAX_LOCKED_COMPONENTS, MAX_LOCKED_CONTENT,
    MAX_LOCKED_DEPENDENCIES_PER_ENTRY, MAX_LOCKED_EXTRACTIONS, MAX_LOCKED_OUTPUTS,
};
pub use path::{MAX_MANAGED_PATH_BYTES, ManagedRelativePath};
pub use receipt::{
    INSTALL_FORMAT_VERSION, INSTALL_RECEIPT_SCHEMA_VERSION, InstallReceipt, InstalledArgument,
    InstalledArtifact, InstalledComponent, InstalledComponentKind, InstalledJavaRequirement,
    InstalledLibrary, InstalledRule, InstalledRuleAction,
};
pub use repair::{
    REPAIR_PLAN_SCHEMA_VERSION, RepairAction, RepairOptions, RepairPlan, RepairResult,
};
pub use spec::{MAX_INSTANCE_NAME_BYTES, NewInstanceSpec};
pub use verification::{
    FindingCode, FindingSeverity, MAX_VERIFICATION_FINDINGS, Repairability, VerificationFinding,
    VerificationMode, VerificationReport,
};
