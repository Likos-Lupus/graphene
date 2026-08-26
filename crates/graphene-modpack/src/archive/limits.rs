//! Named resource limits for pack import/export. All bounds are explicit and finite.

/// Maximum accepted pack archive size in bytes (8 GiB).
pub const MAX_PACK_ARCHIVE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
/// Maximum number of archive entries considered during import (100,000).
pub const MAX_ARCHIVE_ENTRIES: usize = 100_000;
/// Maximum manifest document size in bytes (8 MiB).
pub const MAX_MANIFEST_BYTES: usize = 8 * 1024 * 1024;
/// Maximum archive entry name length in bytes (1,024).
pub const MAX_ENTRY_NAME_BYTES: usize = 1_024;
/// Maximum normalized path depth in components (64).
pub const MAX_PATH_DEPTH: usize = 64;
/// Maximum single expanded entry payload in bytes (2 GiB).
pub const MAX_EXPANDED_ENTRY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Maximum total selected expanded payload in bytes (16 GiB).
pub const MAX_TOTAL_EXPANSION_BYTES: u64 = 16 * 1024 * 1024 * 1024;
/// Maximum managed downloadable files declared by one pack (20,000).
pub const MAX_MANAGED_FILES: usize = 20_000;
/// Maximum total declared managed download bytes (64 GiB).
pub const MAX_DECLARED_MANAGED_BYTES: u64 = 64 * 1024 * 1024 * 1024;
/// Maximum characters in bounded display strings such as pack names.
pub const MAX_DISPLAY_STRING_CHARS: usize = 256;
/// Maximum number of embedded seed entries considered during inspection.
pub const MAX_SEED_ENTRIES: usize = 100_000;
