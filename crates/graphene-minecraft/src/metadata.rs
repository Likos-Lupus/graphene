use crate::{
    Argument, Library, ManagedPath, MinecraftVersionId, MinecraftVersionType, error::mc_error,
};
use graphene_core::{Artifact, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Immutable artifact plus its provider-neutral managed destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedArtifact {
    pub artifact: Artifact,
    pub relative_path: ManagedPath,
}

/// Asset index object normalized by the provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedAssetObject {
    pub logical_name: String,
    pub hash: String,
    pub size: u64,
    pub artifact: ResolvedArtifact,
}

/// Fully enumerated asset model required before install execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedAssets {
    pub index_id: String,
    pub index: ResolvedArtifact,
    pub objects: Vec<ResolvedAssetObject>,
    pub virtual_layout: bool,
    pub map_to_resources: bool,
}

/// Optional client logging configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedLogging {
    pub artifact: ResolvedArtifact,
    pub argument: String,
}

/// Minecraft-declared Java compatibility requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinecraftJavaRequirement {
    pub major_version: u32,
    pub component_hint: Option<String>,
}

/// Provider-normalized version JSON before inheritance is fully resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinecraftVersionMetadata {
    pub id: MinecraftVersionId,
    pub version_type: MinecraftVersionType,
    pub inherits_from: Option<MinecraftVersionId>,
    pub main_class: Option<String>,
    pub client: Option<ResolvedArtifact>,
    pub jvm_args: Vec<Argument>,
    pub game_args: Vec<Argument>,
    pub legacy_minecraft_arguments: Option<String>,
    pub libraries: Vec<Library>,
    pub asset_index: Option<ResolvedArtifact>,
    pub assets_id: Option<String>,
    pub logging: Option<ResolvedLogging>,
    pub java_requirement: Option<MinecraftJavaRequirement>,
    pub release_time: Option<String>,
    pub compliance_level: Option<u32>,
}

/// Parent-first deterministic metadata merge.
pub fn merge_metadata(
    mut parent: MinecraftVersionMetadata,
    child: MinecraftVersionMetadata,
) -> Result<MinecraftVersionMetadata> {
    if parent.id == child.id {
        return Err(mc_error(
            ErrorCode::MinecraftInheritanceCycle,
            "Minecraft metadata cannot inherit from itself",
        ));
    }

    parent.id = child.id;
    parent.version_type = child.version_type;
    // A successful parent-first merge consumes the inheritance edge.
    parent.inherits_from = None;
    if child.main_class.is_some() {
        parent.main_class = child.main_class;
    }
    if child.client.is_some() {
        parent.client = child.client;
    }
    parent.jvm_args.extend(child.jvm_args);
    parent.game_args.extend(child.game_args);
    if child.legacy_minecraft_arguments.is_some() {
        parent.legacy_minecraft_arguments = child.legacy_minecraft_arguments;
    }
    merge_libraries(&mut parent.libraries, child.libraries);
    if child.asset_index.is_some() {
        parent.asset_index = child.asset_index;
    }
    if child.assets_id.is_some() {
        parent.assets_id = child.assets_id;
    }
    if child.logging.is_some() {
        parent.logging = child.logging;
    }
    if child.java_requirement.is_some() {
        parent.java_requirement = child.java_requirement;
    }
    if child.release_time.is_some() {
        parent.release_time = child.release_time;
    }
    if child.compliance_level.is_some() {
        parent.compliance_level = child.compliance_level;
    }

    Ok(parent)
}

fn merge_libraries(parent: &mut Vec<Library>, children: Vec<Library>) {
    let mut positions = BTreeMap::new();

    for (index, library) in parent.iter().enumerate() {
        positions.insert(library.coordinate.library_identity(), index);
    }

    for child in children {
        let identity = child.coordinate.library_identity();
        if let Some(index) = positions.get(&identity).copied() {
            parent[index] = child;
        } else {
            positions.insert(identity, parent.len());
            parent.push(child);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::complete_metadata;
    use crate::{Argument, MavenCoordinate, MinecraftVersionType};

    fn library(version: &str) -> Library {
        Library {
            coordinate: MavenCoordinate::parse(&format!("com.example:demo:{version}"))
                .expect("coordinate"),
            rules: Vec::new(),
            artifact: None,
            classifiers: BTreeMap::new(),
            natives: BTreeMap::new(),
        }
    }

    #[test]
    fn child_library_replaces_parent_by_maven_identity_without_reordering() {
        let base = MinecraftVersionMetadata {
            id: MinecraftVersionId::new("parent").expect("version"),
            version_type: MinecraftVersionType::Release,
            inherits_from: None,
            main_class: Some("Main".into()),
            client: None,
            jvm_args: vec![Argument::Literal("parent".into())],
            game_args: Vec::new(),
            legacy_minecraft_arguments: None,
            libraries: vec![library("1")],
            asset_index: None,
            assets_id: None,
            logging: None,
            java_requirement: None,
            release_time: None,
            compliance_level: None,
        };
        let child = MinecraftVersionMetadata {
            id: MinecraftVersionId::new("child").expect("version"),
            version_type: MinecraftVersionType::Release,
            inherits_from: None,
            main_class: None,
            client: None,
            jvm_args: vec![Argument::Literal("child".into())],
            game_args: Vec::new(),
            legacy_minecraft_arguments: None,
            libraries: vec![library("2")],
            asset_index: None,
            assets_id: None,
            logging: None,
            java_requirement: None,
            release_time: None,
            compliance_level: None,
        };
        let merged = merge_metadata(base, child).expect("merge");
        assert_eq!(
            merged.jvm_args,
            vec![
                Argument::Literal("parent".into()),
                Argument::Literal("child".into())
            ]
        );
        assert_eq!(merged.libraries.len(), 1);
        assert_eq!(merged.libraries[0].coordinate.version, "2");
    }

    #[test]
    fn child_scalar_values_override_parent_values() {
        let mut parent = complete_metadata(Vec::new());
        parent.id = MinecraftVersionId::new("parent").expect("version");
        parent.main_class = Some("ParentMain".into());
        parent.assets_id = Some("parent-assets".into());
        parent.release_time = Some("parent-time".into());

        let child = MinecraftVersionMetadata {
            id: MinecraftVersionId::new("child").expect("version"),
            version_type: MinecraftVersionType::Snapshot,
            inherits_from: None,
            main_class: Some("ChildMain".into()),
            client: None,
            jvm_args: Vec::new(),
            game_args: Vec::new(),
            legacy_minecraft_arguments: None,
            libraries: Vec::new(),
            asset_index: None,
            assets_id: Some("child-assets".into()),
            logging: None,
            java_requirement: None,
            release_time: Some("child-time".into()),
            compliance_level: None,
        };

        let merged = merge_metadata(parent, child).expect("merge");
        assert_eq!(merged.id.as_str(), "child");
        assert_eq!(merged.version_type, MinecraftVersionType::Snapshot);
        assert_eq!(merged.main_class.as_deref(), Some("ChildMain"));
        assert_eq!(merged.assets_id.as_deref(), Some("child-assets"));
        assert_eq!(merged.release_time.as_deref(), Some("child-time"));
        assert_eq!(
            merged.java_requirement.expect("requirement").major_version,
            21
        );
    }
}
