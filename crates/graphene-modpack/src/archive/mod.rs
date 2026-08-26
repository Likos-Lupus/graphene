//! Bounded archive primitives: named limits, pack path policy, and the pack archive index.

pub mod index;
pub mod limits;
pub mod path;

pub use index::{PackArchiveIndex, StreamedEntry};
pub use limits::{
    MAX_ARCHIVE_ENTRIES, MAX_DECLARED_MANAGED_BYTES, MAX_DISPLAY_STRING_CHARS,
    MAX_ENTRY_NAME_BYTES, MAX_EXPANDED_ENTRY_BYTES, MAX_MANAGED_FILES, MAX_MANIFEST_BYTES,
    MAX_PACK_ARCHIVE_BYTES, MAX_PATH_DEPTH, MAX_SEED_ENTRIES, MAX_TOTAL_EXPANSION_BYTES,
};
pub use path::{PackPath, detect_wrapper_root};
