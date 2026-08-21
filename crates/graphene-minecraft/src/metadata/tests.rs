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
