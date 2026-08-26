//! CurseForge manifest adapter: private DTOs plus immediate normalization.

mod dto;
mod normalize;

use crate::archive::PackArchiveIndex;
use std::io::{Read, Seek};

pub(crate) const MANIFEST_NAME: &str = "manifest.json";

/// Recognition requires a present Minecraft modpack manifest with the expected type/version.
pub(super) fn recognizes<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    root: &str,
) -> Result<bool, crate::error::PackError> {
    let name = format!("{root}{MANIFEST_NAME}");
    if index.entry(&name).is_none() {
        return Ok(false);
    }

    let bytes = index.read_manifest(&name)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| crate::error::PackError::manifest("manifest.json is not valid JSON"))?;
    let manifest_type = value
        .get("manifestType")
        .and_then(serde_json::Value::as_str);
    let manifest_version = value
        .get("manifestVersion")
        .and_then(serde_json::Value::as_u64);
    let minecraft = value
        .get("minecraft")
        .map(|m| m.is_object())
        .unwrap_or(false);

    match (manifest_type, manifest_version, minecraft) {
        (Some("minecraftModpack"), Some(1), true) => Ok(true),
        (Some("minecraftModpack"), Some(other), _) => Err(crate::error::PackError::manifest(
            format!("unsupported CurseForge manifest version {other}"),
        )),
        (Some(_), _, _) => Ok(false),
        _ => Err(crate::error::PackError::manifest(
            "manifest.json lacks manifestType/manifestVersion/minecraft",
        )),
    }
}

pub use normalize::normalize;
