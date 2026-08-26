//! Private Prism Launcher / MultiMC manifest DTOs.

use serde::Deserialize;
use std::collections::BTreeMap;

pub(super) const MAX_COMPONENTS: usize = 128;

#[derive(Debug, Deserialize)]
pub(super) struct MultiMcPackDocument {
    #[serde(rename = "formatVersion")]
    pub format_version: u64,
    pub components: Vec<MultiMcComponentRecord>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct MultiMcComponentRecord {
    pub uid: String,
    pub version: Option<String>,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct MultiMcPatchDocument {
    #[serde(rename = "formatVersion")]
    pub format_version: Option<u64>,
    pub uid: String,
    pub version: String,
    #[serde(default)]
    pub requires: Vec<MultiMcRequirementRecord>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct MultiMcRequirementRecord {
    pub uid: String,
    pub equals: Option<String>,
    pub suggests: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
