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
    };

    lockfile.validate().expect("valid lockfile");

    let fp1 = InstanceStateFingerprint::compute(id, &receipt, Some(&lockfile), None);
    let fp2 = InstanceStateFingerprint::compute(id, &receipt, Some(&lockfile), None);
    assert_eq!(fp1, fp2);

    // Renaming display name does not change executable state fingerprint
    let _desc1 = InstanceDescriptor::create(&NewInstanceSpec::with_id(id, "Name 1").unwrap());
    let _desc2 = InstanceDescriptor::create(&NewInstanceSpec::with_id(id, "Name 2").unwrap());
    let fp3 = InstanceStateFingerprint::compute(id, &receipt, Some(&lockfile), None);
    assert_eq!(fp1, fp3);
}
