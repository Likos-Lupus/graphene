use super::*;
use crate::{
    InstallReceipt, InstalledComponent, InstalledComponentKind, InstalledJavaRequirement,
    InstanceDescriptor, NewInstanceSpec,
};
use graphene_core::{ArtifactIntegrity, ArtifactKind, InstanceId};

fn fixture_receipt(id: InstanceId) -> InstallReceipt {
    InstallReceipt {
        schema_version: 2,
        install_format_version: 1,
        instance_id: id,
        requested_version: "1.21.1".to_string(),
        resolved_version: "1.21.1".to_string(),
        components: vec![InstalledComponent {
            uid: "net.minecraft".to_string(),
            version: "1.21.1".to_string(),
            kind: InstalledComponentKind::Minecraft,
            provider: "mojang".to_string(),
            provenance: None,
        }],
        version_type: "release".to_string(),
        main_class: "net.minecraft.client.main.Main".to_string(),
        java_requirement: InstalledJavaRequirement {
            major_version: 21,
            component_hint: Some("java-runtime-gamma".to_string()),
        },
        client: crate::InstalledArtifact {
            path: crate::ManagedRelativePath::new(".minecraft/versions/1.21.1/1.21.1.jar").unwrap(),
            integrity: ArtifactIntegrity::default(),
            expected_size: Some(100),
        },
        libraries: Vec::new(),
        asset_index_id: "21".to_string(),
        asset_index: crate::InstalledArtifact {
            path: crate::ManagedRelativePath::new("shared/assets/indexes/21.json").unwrap(),
            integrity: ArtifactIntegrity::default(),
            expected_size: Some(100),
        },
        assets_root: crate::ManagedRelativePath::new("shared/assets").unwrap(),
        natives_directory: crate::ManagedRelativePath::new(".graphene/natives/1.21.1").unwrap(),
        logging_configuration: None,
        logging_argument: None,
        jvm_arguments: Vec::new(),
        game_arguments: Vec::new(),
    }
}

#[test]
fn lockfile_validation_and_fingerprint_stability() {
    let id = InstanceId::new();
    let receipt = fixture_receipt(id);

    let lockfile = InstanceLockfile {
        schema_version: LOCKFILE_SCHEMA_VERSION,
        instance_id: id,
        minecraft_version: "1.21.1".to_string(),
        components: receipt.components.clone(),
        artifacts: vec![LockedArtifact {
            logical_key: "client".to_string(),
            kind: ArtifactKind::Binary,
            sources: Vec::new(),
            destination: crate::ManagedRelativePath::new(".minecraft/versions/1.21.1/1.21.1.jar")
                .unwrap(),
            scope: LockedMaterializationScope::InstanceMutable,
            integrity: ArtifactIntegrity::default(),
            expected_size: Some(100),
        }],
        native_extractions: Vec::new(),
        generated_outputs: Vec::new(),
        content: Vec::new(),
    };

    lockfile.validate().expect("valid lockfile");

    let fp1 = InstanceStateFingerprint::compute(id, &receipt, Some(&lockfile), None);
    let fp2 = InstanceStateFingerprint::compute(id, &receipt, Some(&lockfile), None);
    assert_eq!(fp1, fp2);

    // Adding content changes the fingerprint
    let mut lockfile2 = lockfile.clone();
    lockfile2.content.push(LockedContentEntry {
        entry_id: "entry-1".to_string(),
        kind: "MOD".to_string(),
        provider: Some("modrinth".to_string()),
        project_id: Some("sodium".to_string()),
        version_id: Some("0.5.8".to_string()),
        file_id: Some("file-1".to_string()),
        artifact_logical_key: "mod:sodium".to_string(),
        destination: crate::ManagedRelativePath::new(".minecraft/mods/sodium.jar").unwrap(),
        enabled: true,
        dependencies: Vec::new(),
    });
    lockfile2.validate().expect("valid schema 2 lockfile");

    let fp_content = InstanceStateFingerprint::compute(id, &receipt, Some(&lockfile2), None);
    assert_ne!(fp1, fp_content);

    // Schema 1 deserialization compatibility without content field
    let schema1_json = r#"{
        "schema_version": 1,
        "instance_id": "00000000-0000-0000-0000-000000000001",
        "minecraft_version": "1.21.1",
        "components": [{
            "uid": "net.minecraft",
            "version": "1.21.1",
            "kind": "Minecraft",
            "provider": "mojang"
        }],
        "artifacts": [],
        "native_extractions": [],
        "generated_outputs": []
    }"#;
    let schema1_lock: InstanceLockfile =
        serde_json::from_str(schema1_json).expect("deserializes schema 1");
    assert_eq!(schema1_lock.schema_version, 1);
    assert!(schema1_lock.content.is_empty());
    schema1_lock.validate().expect("valid schema 1 lockfile");

    // Renaming display name does not change executable state fingerprint
    let _desc1 = InstanceDescriptor::create(&NewInstanceSpec::with_id(id, "Name 1").unwrap());
    let _desc2 = InstanceDescriptor::create(&NewInstanceSpec::with_id(id, "Name 2").unwrap());
    let fp3 = InstanceStateFingerprint::compute(id, &receipt, Some(&lockfile), None);
    assert_eq!(fp1, fp3);
}
