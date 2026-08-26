//! `.mrpack` format adapter: private DTOs plus immediate normalization.

mod dto;
mod normalize;

use crate::archive::PackArchiveIndex;
use std::io::{Read, Seek};

pub(crate) const INDEX_NAME: &str = "modrinth.index.json";

/// Recognition requires a present, structurally valid Modrinth index; malformed candidates fail.
pub(super) fn recognizes<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    root: &str,
) -> Result<bool, crate::error::PackError> {
    let name = format!("{root}{INDEX_NAME}");

    if index.entry(&name).is_none() {
        return Ok(false);
    }

    let bytes = index.read_manifest(&name)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| crate::error::PackError::manifest("modrinth.index.json is not valid JSON"))?;
    let version = value
        .get("formatVersion")
        .and_then(serde_json::Value::as_u64);
    let game = value.get("game").and_then(serde_json::Value::as_str);

    match (version, game) {
        (Some(_), Some(game)) => Ok(game == "minecraft"),
        _ => Err(crate::error::PackError::manifest(
            "modrinth.index.json lacks formatVersion/game",
        )),
    }
}

pub use normalize::normalize;
