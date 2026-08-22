use serde::{Deserialize, Serialize};

pub(crate) const LAYOUT_VERSION: u32 = 1;
pub(crate) const MARKER_FILE: &str = ".graphene-layout.json";

pub(crate) const DIRECTORIES: &[&str] = &[
    "config",
    "config/accounts",
    "instances",
    "instances/.locks",
    "instances/.staging",
    "instances/.trash",
    "shared/libraries",
    "shared/assets",
    "shared/runtimes",
    "shared/metadata",
    "cache/http",
    "cache/downloads",
    "cache/downloads/temporary",
    "cache/objects",
    "database",
    "logs",
];

pub const INSTANCE_LOCKS_DIR: &str = ".locks";
pub const INSTANCE_STAGING_DIR: &str = ".staging";
pub const INSTANCE_TRASH_DIR: &str = ".trash";

/// Returns whether a directory name inside `instances/` represents Graphene infrastructure rather
/// than an instance payload.
#[must_use]
pub fn is_instance_infrastructure_name(name: &str) -> bool {
    matches!(
        name,
        INSTANCE_LOCKS_DIR
            | INSTANCE_STAGING_DIR
            | INSTANCE_TRASH_DIR
            | ".install-locks"
            | ".DS_Store"
    ) || name.starts_with('.')
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct LayoutMarker {
    pub layout_version: u32,
}
