use crate::{
    local::metadata::descriptor::{ModMetadataSource, NormalizedModDescriptor},
    model::version::EnvironmentSupport,
};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use serde_json::Value;

pub fn parse_mcmod_info(bytes: &[u8]) -> Result<Vec<NormalizedModDescriptor>> {
    let root: Value = serde_json::from_slice(bytes).map_err(|source| {
        GrapheneError::new(
            ErrorCode::LoaderMetadataInvalid,
            ErrorKind::Minecraft,
            "failed to parse mcmod.info",
        )
        .with_source(source)
    })?;

    let array = if let Some(arr) = root.as_array() {
        arr
    } else if let Some(arr) = root.get("modList").and_then(Value::as_array) {
        arr
    } else {
        return Err(GrapheneError::new(
            ErrorCode::LoaderMetadataInvalid,
            ErrorKind::Minecraft,
            "mcmod.info does not contain a mod list array",
        ));
    };

    let mut descriptors = Vec::new();
    for item in array {
        if let Some(mod_id) = item
            .as_object()
            .and_then(|obj| obj.get("modid"))
            .and_then(Value::as_str)
        {
            let obj = item.as_object().expect("checked object above");
            let name = obj
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(mod_id)
                .to_string();
            let version = obj
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or("0.0.0")
                .to_string();
            let description = obj
                .get("description")
                .and_then(Value::as_str)
                .map(String::from);
            let mut authors = Vec::new();
            if let Some(arr) = obj.get("authorList").and_then(Value::as_array) {
                for a in arr {
                    if let Some(s) = a.as_str() {
                        authors.push(s.to_string());
                    }
                }
            }

            descriptors.push(NormalizedModDescriptor {
                mod_id: mod_id.to_string(),
                name,
                version,
                description,
                authors,
                environment: EnvironmentSupport::Both,
                dependencies: Vec::new(),
                source: ModMetadataSource::LegacyMcModInfo,
            });
        }
    }

    if descriptors.is_empty() {
        return Err(GrapheneError::new(
            ErrorCode::LoaderMetadataInvalid,
            ErrorKind::Minecraft,
            "mcmod.info contained no valid mod entries",
        ));
    }

    Ok(descriptors)
}
