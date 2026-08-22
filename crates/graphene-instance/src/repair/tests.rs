use super::*;
use crate::{
    InstallReceipt, InstalledComponent, InstalledComponentKind, InstalledJavaRequirement,
    InstanceStateFingerprint, VerificationMode,
};
use graphene_core::{ArtifactIntegrity, InstanceId};

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
fn repair_plan_noop_and_serialization() {
    let id = InstanceId::new();
    let receipt = fixture_receipt(id);
    let fingerprint = InstanceStateFingerprint::compute(id, &receipt, None, None);

    let plan = RepairPlan {
        schema_version: REPAIR_PLAN_SCHEMA_VERSION,
        instance_id: id,
        base_state_fingerprint: fingerprint,
        verification_mode: VerificationMode::Full,
        ordered_actions: Vec::new(),
        estimated_download_bytes: 0,
        requires_network: false,
        requires_tool_java: false,
        residual_diagnostics: Vec::new(),
    };

    assert!(plan.is_noop());

    let json = serde_json::to_string(&plan).unwrap();
    let parsed: RepairPlan = serde_json::from_str(&json).unwrap();
    assert_eq!(plan, parsed);
}
