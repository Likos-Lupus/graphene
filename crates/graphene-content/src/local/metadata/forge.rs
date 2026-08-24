use crate::{
    local::metadata::descriptor::{LocalModDependency, ModMetadataSource, NormalizedModDescriptor},
    model::{dependency::DependencyRelation, version::EnvironmentSupport},
};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use toml::Value;

pub fn parse_forge_mods_toml(
    bytes: &[u8],
    source_kind: ModMetadataSource,
) -> Result<Vec<NormalizedModDescriptor>> {
    let toml_str = std::str::from_utf8(bytes).map_err(|source| {
        GrapheneError::new(
            ErrorCode::LoaderMetadataInvalid,
            ErrorKind::Minecraft,
            "mods.toml content is not valid UTF-8",
        )
        .with_source(source)
    })?;

    let root: Value = toml::from_str(toml_str).map_err(|source| {
        GrapheneError::new(
            ErrorCode::LoaderMetadataInvalid,
            ErrorKind::Minecraft,
            "failed to parse mods.toml",
        )
        .with_source(source)
    })?;

    let root_table = root.as_table().ok_or_else(|| {
        GrapheneError::new(
            ErrorCode::LoaderMetadataInvalid,
            ErrorKind::Minecraft,
            "mods.toml root must be a table",
        )
    })?;

    let mods_array = root_table
        .get("mods")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            GrapheneError::new(
                ErrorCode::LoaderMetadataInvalid,
                ErrorKind::Minecraft,
                "mods.toml missing [[mods]] array",
            )
        })?;

    let mut descriptors = Vec::new();

    // Dependencies can be in root_table["dependencies"] as a table of mod_id -> array of dep tables
    let all_deps_table = root_table.get("dependencies").and_then(Value::as_table);

    for mod_val in mods_array {
        let mod_table = match mod_val.as_table() {
            Some(t) => t,
            None => continue,
        };

        let mod_id = match mod_table.get("modId").and_then(Value::as_str) {
            Some(id) => id.to_string(),
            None => continue,
        };

        let name = mod_table
            .get("displayName")
            .and_then(Value::as_str)
            .unwrap_or(&mod_id)
            .to_string();

        let version = mod_table
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("0.0.0")
            .to_string();

        let description = mod_table
            .get("description")
            .and_then(Value::as_str)
            .map(String::from);

        let authors = mod_table
            .get("authors")
            .and_then(Value::as_str)
            .map(|s| {
                s.split(',')
                    .map(|a| a.trim().to_string())
                    .filter(|a| !a.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let mut dependencies = Vec::new();

        if let Some(dep_array) = all_deps_table
            .and_then(|t| t.get(&mod_id))
            .and_then(Value::as_array)
        {
            for dep_val in dep_array {
                if let Some((dep_mod_id, dt)) = dep_val
                    .as_table()
                    .and_then(|dt| dt.get("modId").and_then(Value::as_str).map(|id| (id, dt)))
                {
                    let mandatory = dt.get("mandatory").and_then(Value::as_bool).unwrap_or(true);
                    let version_range = dt
                        .get("versionRange")
                        .and_then(Value::as_str)
                        .map(String::from);
                    let relation = if mandatory {
                        DependencyRelation::Required
                    } else {
                        DependencyRelation::Optional
                    };
                    dependencies.push(LocalModDependency {
                        mod_id: dep_mod_id.to_string(),
                        version_range,
                        relation,
                    });
                }
            }
        }

        descriptors.push(NormalizedModDescriptor {
            mod_id,
            name,
            version,
            description,
            authors,
            environment: EnvironmentSupport::Both,
            dependencies,
            source: source_kind,
        });
    }

    if descriptors.is_empty() {
        return Err(GrapheneError::new(
            ErrorCode::LoaderMetadataInvalid,
            ErrorKind::Minecraft,
            "mods.toml contained no valid [[mods]] entries",
        ));
    }

    Ok(descriptors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_forge_mods() {
        let toml = r#"
modLoader="javafml"
loaderVersion="[47,)"
license="LGPL-3.0"

[[mods]]
modId="jei"
version="15.2.0.27"
displayName="Just Enough Items"
authors="mezz"
description="JEI is an item and recipe viewing mod for Minecraft."

[[dependencies.jei]]
modId="forge"
mandatory=true
versionRange="[47,)"
ordering="NONE"
side="BOTH"

[[dependencies.jei]]
modId="minecraft"
mandatory=true
versionRange="[1.20.1,1.21)"
ordering="NONE"
side="BOTH"
"#;

        let descriptors = parse_forge_mods_toml(toml.as_bytes(), ModMetadataSource::Forge).unwrap();
        assert_eq!(descriptors.len(), 1);
        let desc = &descriptors[0];
        assert_eq!(desc.mod_id, "jei");
        assert_eq!(desc.name, "Just Enough Items");
        assert_eq!(desc.version, "15.2.0.27");
        assert_eq!(desc.dependencies.len(), 2);
        assert_eq!(desc.source, ModMetadataSource::Forge);
    }
}
