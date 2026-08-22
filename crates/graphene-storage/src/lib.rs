//! Graphene-managed storage root, layout, cache paths, and commit primitives.
//!
//! Filesystem paths are explicitly rooted per engine. There is no process-global data directory.

mod account;
mod atomic;
mod cache;
mod instance;
mod layout;
mod root;
mod runtime;
mod temp;

pub use account::{ACCOUNT_RECORD_SCHEMA_VERSION, AccountDocumentStore};
pub use cache::{CacheAddress, CacheAlgorithm};
pub use instance::{
    InstanceExclusiveLease, InstanceLeaseStore, InstancePaths, InstanceSharedLease,
    InstanceStagingTree, InstanceTrash, MAX_CLONE_TOTAL_BYTES, MAX_CONFIG_BYTES,
    MAX_DESCRIPTOR_BYTES, MAX_LOCKFILE_BYTES, MAX_RECEIPT_BYTES, clone_instance_tree,
    read_document_bounded, read_json_bounded, write_document_atomic, write_json_atomic,
};
pub use layout::{
    INSTANCE_LOCKS_DIR, INSTANCE_STAGING_DIR, INSTANCE_TRASH_DIR, is_instance_infrastructure_name,
};
pub use root::DataRoot;
pub use runtime::ManagedRuntimeStore;
