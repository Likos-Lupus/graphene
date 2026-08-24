use crate::{
    id::{ContentEntryId, ContentProviderId},
    local::fingerprint::ContentInventoryFingerprint,
    model::{
        compatibility::{ContentSide, InstanceContentContext},
        kind::ContentKind,
    },
    plan::model::{CONTENT_PLAN_SCHEMA_VERSION, ContentMutationPlan, PlannedContentEntry},
};
use graphene_core::InstanceId;
use graphene_instance::{InstanceStateFingerprint, ManagedRelativePath};

#[test]
fn validates_empty_plan() {
    let id = InstanceId::new();
    let plan = ContentMutationPlan {
        schema_version: CONTENT_PLAN_SCHEMA_VERSION,
        instance_id: id,
        base_state_fingerprint: InstanceStateFingerprint::compute(
            id,
            &crate::tests::dummy_receipt(id),
            None,
            None,
        ),
        base_inventory_fingerprint: ContentInventoryFingerprint::compute(std::iter::empty()),
        context: InstanceContentContext {
            instance_id: id,
            minecraft_version: "1.21.1".to_string(),
            loader: None,
            exact_loader_version: None,
            side: ContentSide::Client,
        },
        requested_actions: Vec::new(),
        planned_entries: Vec::new(),
        filesystem_actions: Vec::new(),
        artifacts_to_acquire: Vec::new(),
        resulting_lockfile_entries: Vec::new(),
        diagnostics: Vec::new(),
        estimated_download_bytes: 0,
    };

    assert!(plan.validate().is_ok());
    assert!(plan.is_noop());
}

#[test]
fn rejects_duplicate_destinations() {
    let id = InstanceId::new();
    let entry1 = PlannedContentEntry {
        entry_id: ContentEntryId::generate(),
        kind: ContentKind::Mod,
        provider: Some(ContentProviderId::new("modrinth").unwrap()),
        project_id: Some("p1".to_string()),
        version_id: Some("v1".to_string()),
        file_id: Some("f1".to_string()),
        artifact_logical_key: "k1".to_string(),
        destination: ManagedRelativePath::new(".minecraft/mods/foo.jar").unwrap(),
        enabled: true,
        dependencies: Vec::new(),
    };

    let entry2 = PlannedContentEntry {
        entry_id: ContentEntryId::generate(),
        kind: ContentKind::Mod,
        provider: Some(ContentProviderId::new("modrinth").unwrap()),
        project_id: Some("p2".to_string()),
        version_id: Some("v2".to_string()),
        file_id: Some("f2".to_string()),
        artifact_logical_key: "k2".to_string(),
        destination: ManagedRelativePath::new(".minecraft/mods/foo.jar").unwrap(),
        enabled: true,
        dependencies: Vec::new(),
    };

    let plan = ContentMutationPlan {
        schema_version: CONTENT_PLAN_SCHEMA_VERSION,
        instance_id: id,
        base_state_fingerprint: InstanceStateFingerprint::compute(
            id,
            &crate::tests::dummy_receipt(id),
            None,
            None,
        ),
        base_inventory_fingerprint: ContentInventoryFingerprint::compute(std::iter::empty()),
        context: InstanceContentContext {
            instance_id: id,
            minecraft_version: "1.21.1".to_string(),
            loader: None,
            exact_loader_version: None,
            side: ContentSide::Client,
        },
        requested_actions: Vec::new(),
        planned_entries: vec![entry1, entry2],
        filesystem_actions: Vec::new(),
        artifacts_to_acquire: Vec::new(),
        resulting_lockfile_entries: Vec::new(),
        diagnostics: Vec::new(),
        estimated_download_bytes: 0,
    };

    assert!(plan.validate().is_err());
}
