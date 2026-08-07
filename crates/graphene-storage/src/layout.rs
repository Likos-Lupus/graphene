use serde::{Deserialize, Serialize};

pub(crate) const LAYOUT_VERSION: u32 = 1;
pub(crate) const MARKER_FILE: &str = ".graphene-layout.json";

pub(crate) const DIRECTORIES: &[&str] = &[
    "config",
    "instances",
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

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct LayoutMarker {
    pub layout_version: u32,
}
