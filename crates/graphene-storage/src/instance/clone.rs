use graphene_core::Result;
use graphene_platform::copy_contained_tree;
use std::path::Path;

/// Maximum default byte budget for an instance clone (100 GiB).
pub const MAX_CLONE_TOTAL_BYTES: u64 = 100 * 1024 * 1024 * 1024;

/// Copies an entire instance tree into a staging tree without following symlinks.
///
/// Rejects symlinks and special files. All file writes are streamed and synced.
pub fn clone_instance_tree(
    source_root: &Path,
    staging_root: &Path,
    max_total_bytes: u64,
) -> Result<u64> {
    copy_contained_tree(source_root, staging_root, max_total_bytes)
}
