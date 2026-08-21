//! Create-only instance identity and durable, provider-neutral installation metadata.
//!
//! The crate root is a facade. Schema ownership and validation are split by instance concept;
//! filesystem mutation, Minecraft resolution, provider parsing, and launch behavior remain outside
//! this bounded context.

mod committed;
mod descriptor;
mod error;
mod path;
mod receipt;
mod spec;

pub use committed::CommittedInstance;
pub use descriptor::{INSTANCE_SCHEMA_VERSION, InstanceDescriptor};
pub use path::{MAX_MANAGED_PATH_BYTES, ManagedRelativePath};
pub use receipt::{
    INSTALL_FORMAT_VERSION, INSTALL_RECEIPT_SCHEMA_VERSION, InstallReceipt, InstalledArgument,
    InstalledArtifact, InstalledComponent, InstalledComponentKind, InstalledJavaRequirement,
    InstalledLibrary, InstalledRule, InstalledRuleAction,
};
pub use spec::{MAX_INSTANCE_NAME_BYTES, NewInstanceSpec};
