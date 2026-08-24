use graphene_core::{ArtifactIntegrity, InstanceId};
use graphene_instance::{
    InstallReceipt, InstalledArtifact, InstalledComponent, InstalledComponentKind,
    InstalledJavaRequirement, ManagedRelativePath,
};

#[must_use]
pub fn dummy_receipt(id: InstanceId) -> InstallReceipt {
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
            component_hint: None,
        },
        client: InstalledArtifact {
            path: ManagedRelativePath::new(".minecraft/versions/1.21.1/1.21.1.jar").unwrap(),
            integrity: ArtifactIntegrity::default(),
            expected_size: Some(100),
        },
        libraries: Vec::new(),
        asset_index_id: "21".to_string(),
        asset_index: InstalledArtifact {
            path: ManagedRelativePath::new("shared/assets/indexes/21.json").unwrap(),
            integrity: ArtifactIntegrity::default(),
            expected_size: Some(100),
        },
        assets_root: ManagedRelativePath::new("shared/assets").unwrap(),
        natives_directory: ManagedRelativePath::new(".graphene/natives/1.21.1").unwrap(),
        logging_configuration: None,
        logging_argument: None,
        jvm_arguments: Vec::new(),
        game_arguments: Vec::new(),
    }
}
