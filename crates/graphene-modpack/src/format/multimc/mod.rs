//! Prism Launcher / MultiMC adapter with private foreign DTOs and Graphene-owned output.

mod dto;
mod normalize;

use crate::archive::PackArchiveIndex;
use std::io::{Read, Seek};

pub(crate) const PACK_NAME: &str = "mmc-pack.json";

/// Recognition requires `mmc-pack.json`, a component list, and one supported content root.
pub(super) fn recognizes<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    root: &str,
) -> Result<bool, crate::error::PackError> {
    let name = format!("{root}{PACK_NAME}");

    if index.entry(&name).is_none() {
        return Ok(false);
    }

    let modern_root = format!("{root}.minecraft/");
    let historical_root = format!("{root}minecraft/");
    let has_content_root = index.entries().iter().any(|entry| {
        entry.name == modern_root
            || entry.name.starts_with(&modern_root)
            || entry.name == historical_root
            || entry.name.starts_with(&historical_root)
    });

    if !has_content_root {
        return Ok(false);
    }

    let bytes = index.read_manifest(&name)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| crate::error::PackError::manifest("mmc-pack.json is not valid JSON"))?;

    match (
        value
            .get("formatVersion")
            .and_then(serde_json::Value::as_u64),
        value.get("components").map(serde_json::Value::is_array),
    ) {
        (Some(_), Some(true)) => Ok(true),
        _ => Err(crate::error::PackError::manifest(
            "mmc-pack.json lacks formatVersion/components",
        )),
    }
}

pub use normalize::{EmbeddedMod, NormalizedMultiMcPack, normalize};
