use super::*;
use crate::{
    InstallReceipt, InstalledComponent, InstalledComponentKind, InstalledJavaRequirement,
    InstanceStateFingerprint,
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
fn verification_report_health_and_repairability_logic() {
    let id = InstanceId::new();
    let receipt = fixture_receipt(id);
    let fingerprint = InstanceStateFingerprint::compute(id, &receipt, None, None);

    let clean_report = VerificationReport {
        instance_id: id,
        mode: VerificationMode::Quick,
        state_fingerprint: fingerprint,
        repairability: Repairability::Healthy,
        findings: Vec::new(),
        summary: "Instance is healthy".to_string(),
    };
    assert!(clean_report.is_healthy());
    assert!(clean_report.is_repairable());

    let error_finding = VerificationFinding::new(
        FindingCode::ManagedFileMissing,
        FindingSeverity::Error,
        "Missing jar",
        true,
    );
    let damaged_report = VerificationReport {
        instance_id: id,
        mode: VerificationMode::Full,
        state_fingerprint: fingerprint,
        repairability: Repairability::RepairableWithNetwork,
        findings: vec![error_finding],
        summary: "Damaged files found".to_string(),
    };
    assert!(!damaged_report.is_healthy());
    assert!(damaged_report.is_repairable());
}
