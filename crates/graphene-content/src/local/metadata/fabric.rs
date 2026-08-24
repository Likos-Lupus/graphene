use crate::{
    local::metadata::descriptor::{LocalModDependency, ModMetadataSource, NormalizedModDescriptor},
    model::{dependency::DependencyRelation, version::EnvironmentSupport},
};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use serde_json::Value;

pub fn parse_fabric_mod_json(bytes: &[u8]) -> Result<NormalizedModDescriptor> {
    let root: Value = serde_json::from_slice(bytes).map_err(|source| {
        GrapheneError::new(
            ErrorCode::LoaderMetadataInvalid,
            ErrorKind::Minecraft,
            "failed to parse fabric.mod.json",
        )
        .with_source(source)
    })?;

    let obj = root.as_object().ok_or_else(|| {
        GrapheneError::new(
            ErrorCode::LoaderMetadataInvalid,
            ErrorKind::Minecraft,
            "fabric.mod.json root must be a JSON object",
        )
    })?;

    let mod_id = obj
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            GrapheneError::new(
                ErrorCode::LoaderMetadataInvalid,
                ErrorKind::Minecraft,
                "fabric.mod.json missing required 'id' field",
            )
        })?
        .to_string();

    let name = obj
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(&mod_id)
        .to_string();

    let version = obj
        .get("version")
        .and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            Value::Object(_) | Value::Array(_) => None,
            other => Some(other.to_string()),
        })
        .unwrap_or_else(|| "0.0.0".to_string());

    let description = obj
        .get("description")
        .and_then(Value::as_str)
        .map(String::from);

    let mut authors = Vec::new();
    if let Some(authors_val) = obj.get("authors") {
        if let Some(arr) = authors_val.as_array() {
            for a in arr {
                if let Some(s) = a.as_str() {
                    authors.push(s.to_string());
                } else if let Some(name_str) = a
                    .as_object()
                    .and_then(|a_obj| a_obj.get("name"))
                    .and_then(Value::as_str)
                {
                    authors.push(name_str.to_string());
                }
            }
        } else if let Some(s) = authors_val.as_str() {
            authors.push(s.to_string());
        }
    }

    let environment = match obj.get("environment").and_then(Value::as_str) {
        Some("client") => EnvironmentSupport::ClientOnly,
        Some("server") => EnvironmentSupport::ServerOnly,
        _ => EnvironmentSupport::Both,
    };

    let mut dependencies = Vec::new();

    // Helper to parse dependency maps
    let parse_dep_map =
        |key: &str, relation: DependencyRelation, deps: &mut Vec<LocalModDependency>| {
            if let Some(map) = obj.get(key).and_then(Value::as_object) {
                for (dep_id, ver_val) in map {
                    let range = match ver_val {
                        Value::String(s) => Some(s.clone()),
                        Value::Array(arr) => {
                            let parts: Vec<String> = arr
                                .iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect();
                            if parts.is_empty() {
                                None
                            } else {
                                Some(parts.join(" || "))
                            }
                        }
                        _ => None,
                    };
                    deps.push(LocalModDependency {
                        mod_id: dep_id.clone(),
                        version_range: range,
                        relation,
                    });
                }
            }
        };

    parse_dep_map("depends", DependencyRelation::Required, &mut dependencies);
    parse_dep_map(
        "recommends",
        DependencyRelation::Optional,
        &mut dependencies,
    );
    parse_dep_map("suggests", DependencyRelation::Optional, &mut dependencies);
    parse_dep_map(
        "conflicts",
        DependencyRelation::Incompatible,
        &mut dependencies,
    );
    parse_dep_map(
        "breaks",
        DependencyRelation::Incompatible,
        &mut dependencies,
    );

    Ok(NormalizedModDescriptor {
        mod_id,
        name,
        version,
        description,
        authors,
        environment,
        dependencies,
        source: ModMetadataSource::Fabric,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_fabric_mod() {
        let json = r#"{
            "schemaVersion": 1,
            "id": "sodium",
            "version": "0.5.8",
            "name": "Sodium",
            "description": "Modern rendering engine",
            "authors": ["JellySquid", { "name": "FlashyReese" }],
            "environment": "client",
            "depends": {
                "fabricloader": ">=0.15.0",
                "minecraft": "1.20.1"
            },
            "breaks": {
                "optifine": "*"
            }
        }"#;

        let descriptor = parse_fabric_mod_json(json.as_bytes()).unwrap();
        assert_eq!(descriptor.mod_id, "sodium");
        assert_eq!(descriptor.name, "Sodium");
        assert_eq!(descriptor.version, "0.5.8");
        assert_eq!(descriptor.environment, EnvironmentSupport::ClientOnly);
        assert_eq!(descriptor.authors, vec!["JellySquid", "FlashyReese"]);
        assert_eq!(descriptor.dependencies.len(), 3);
        assert_eq!(descriptor.source, ModMetadataSource::Fabric);
    }
}
