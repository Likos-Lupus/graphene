//! Graphene pack v1 manifest recognition and strict normalization.

mod dto;
mod normalize;

/// Light structural recognition of a Graphene pack manifest payload.
pub(super) fn recognizes_manifest(bytes: &[u8]) -> Result<bool, crate::error::PackError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| crate::error::PackError::manifest("graphene.pack.json is not valid JSON"))?;
    let schema = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            crate::error::PackError::manifest("graphene.pack.json lacks schema_version")
        })?;
    Ok(schema == 1)
}

pub use normalize::normalize;
