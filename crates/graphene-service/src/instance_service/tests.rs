use super::*;
use crate::builder::Graphene;
use graphene_core::{ArtifactIntegrity, InstanceId};
use graphene_instance::{
    InstallReceipt, InstalledComponent, InstalledComponentKind, InstalledJavaRequirement,
    InstanceDescriptor, NewInstanceSpec,
};
use graphene_storage::{write_document_atomic, write_json_atomic};

async fn test_service() -> (tempfile::TempDir, InstanceService) {
    let temp = tempfile::tempdir().unwrap();
    let engine = Graphene::builder(temp.path()).build().await.unwrap();
    let service = engine.instances();
    (temp, service)
}

fn fixture_receipt(id: InstanceId, version: &str) -> InstallReceipt {
    InstallReceipt {
        schema_version: 2,
        install_format_version: 1,
        instance_id: id,
        requested_version: version.to_string(),
        resolved_version: version.to_string(),
        components: vec![InstalledComponent {
            uid: "net.minecraft".to_string(),
            version: version.to_string(),
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
        client: graphene_instance::InstalledArtifact {
            path: graphene_instance::ManagedRelativePath::new(format!(
                ".minecraft/versions/{version}/{version}.jar"
            ))
            .unwrap(),
            integrity: ArtifactIntegrity::default(),
            expected_size: Some(100),
        },
        libraries: Vec::new(),
        asset_index_id: "21".to_string(),
        asset_index: graphene_instance::InstalledArtifact {
            path: graphene_instance::ManagedRelativePath::new("shared/assets/indexes/21.json")
                .unwrap(),
            integrity: ArtifactIntegrity::default(),
            expected_size: Some(100),
        },
        assets_root: graphene_instance::ManagedRelativePath::new("shared/assets").unwrap(),
        natives_directory: graphene_instance::ManagedRelativePath::new(format!(
            ".graphene/natives/{version}"
        ))
        .unwrap(),
        logging_configuration: None,
        logging_argument: None,
        jvm_arguments: Vec::new(),
        game_arguments: Vec::new(),
    }
}

#[tokio::test]
async fn inventory_scans_ready_legacy_and_invalid_deterministically() {
    let (_temp, service) = test_service().await;
    let repo = service.repository();

    let id1 = InstanceId::from_bytes([1; 16]);
    let id2 = InstanceId::from_bytes([2; 16]);
    let id3 = InstanceId::from_bytes([3; 16]);

    // Create id1 as Legacy (valid descriptor + receipt, missing lockfile)
    let desc1 = InstanceDescriptor::create(&NewInstanceSpec::with_id(id1, "Instance 1").unwrap());
    let receipt1 = fixture_receipt(id1, "1.21");
    write_json_atomic(&repo.paths().instance_descriptor_path(id1), &desc1).unwrap();
    write_json_atomic(&repo.paths().install_receipt_path(id1), &receipt1).unwrap();

    // Create id2 as Ready (valid descriptor + receipt + lockfile)
    let desc2 = InstanceDescriptor::create(&NewInstanceSpec::with_id(id2, "Instance 2").unwrap());
    let receipt2 = fixture_receipt(id2, "1.21.1");
    write_json_atomic(&repo.paths().instance_descriptor_path(id2), &desc2).unwrap();
    write_json_atomic(&repo.paths().install_receipt_path(id2), &receipt2).unwrap();
    write_document_atomic(&repo.paths().lockfile_path(id2), b"{}").unwrap();

    // Create id3 as Invalid (corrupt json)
    std::fs::create_dir_all(repo.paths().instance_root(id3)).unwrap();
    write_document_atomic(
        &repo.paths().instance_descriptor_path(id3),
        b"corrupt json payload",
    )
    .unwrap();

    let list = service.list().await.unwrap();
    assert_eq!(list.len(), 3);

    // Deterministic sorted order by UUID bytes
    assert_eq!(list[0].instance_id, id1);
    assert_eq!(list[0].status, graphene_instance::InstanceStatus::Legacy);
    assert_eq!(list[0].display_name.as_deref(), Some("Instance 1"));

    assert_eq!(list[1].instance_id, id2);
    assert_eq!(list[1].status, graphene_instance::InstanceStatus::Ready);
    assert_eq!(list[1].display_name.as_deref(), Some("Instance 2"));

    assert_eq!(list[2].instance_id, id3);
    assert_eq!(list[2].status, graphene_instance::InstanceStatus::Invalid);
    assert!(list[2].error.is_some());
}

#[tokio::test]
async fn config_hierarchy_and_updates() {
    let (_temp, service) = test_service().await;
    let repo = service.repository();
    let id = InstanceId::new();

    let desc = InstanceDescriptor::create(&NewInstanceSpec::with_id(id, "Config Test").unwrap());
    let receipt = fixture_receipt(id, "1.21");
    write_json_atomic(&repo.paths().instance_descriptor_path(id), &desc).unwrap();
    write_json_atomic(&repo.paths().install_receipt_path(id), &receipt).unwrap();

    // Default effective config is empty
    let effective = service.effective_config(id).await.unwrap();
    assert_eq!(effective.jvm_args, Vec::<String>::new());

    // Update global defaults
    let global_patch = graphene_instance::InstanceConfigPatch {
        jvm_args: graphene_instance::SettingUpdate::Set(vec!["-Xmx2G".to_string()]),
        resolution: graphene_instance::SettingUpdate::Set(graphene_instance::InstanceResolution {
            width: 1280,
            height: 720,
        }),
        ..Default::default()
    };
    service.update_global_defaults(global_patch).await.unwrap();

    let effective2 = service.effective_config(id).await.unwrap();
    assert_eq!(effective2.jvm_args, vec!["-Xmx2G"]);
    assert_eq!(
        effective2.resolution,
        Some(graphene_instance::InstanceResolution {
            width: 1280,
            height: 720
        })
    );

    // Update instance override
    let inst_patch = graphene_instance::InstanceConfigPatch {
        resolution: graphene_instance::SettingUpdate::Set(graphene_instance::InstanceResolution {
            width: 1920,
            height: 1080,
        }),
        ..Default::default()
    };
    service.update_config(id, inst_patch).await.unwrap();

    let effective3 = service.effective_config(id).await.unwrap();
    assert_eq!(effective3.jvm_args, vec!["-Xmx2G"]);
    assert_eq!(
        effective3.resolution,
        Some(graphene_instance::InstanceResolution {
            width: 1920,
            height: 1080
        })
    );
}

#[tokio::test]
async fn stale_launch_plan_is_rejected_on_state_change() {
    let (temp, service) = test_service().await;
    let repo = service.repository();
    let id = InstanceId::new();

    let desc = InstanceDescriptor::create(&NewInstanceSpec::with_id(id, "Stale Test").unwrap());
    let receipt = fixture_receipt(id, "1.21.1");
    write_json_atomic(&repo.paths().instance_descriptor_path(id), &desc).unwrap();
    write_json_atomic(&repo.paths().install_receipt_path(id), &receipt).unwrap();

    let launch_service = crate::launch_service::LaunchService::new(service.context().clone());
    let java = graphene_java::JavaRuntime {
        major_version: 21,
        vendor: graphene_java::JavaVendor::Adoptium,
        version: "21.0.3".to_string(),
        architecture: graphene_java::JavaArchitecture::current(),
        executable: temp.path().join("java_fake"),
        java_home: None,
    };
    std::fs::write(&java.executable, b"fake java").unwrap();
    std::fs::create_dir_all(repo.paths().instance_root(id).join(".minecraft")).unwrap();
    std::fs::create_dir_all(
        repo.paths()
            .instance_root(id)
            .join(".graphene/natives/1.21.1"),
    )
    .unwrap();
    std::fs::create_dir_all(temp.path().join("shared/assets")).unwrap();
    std::fs::create_dir_all(temp.path().join("shared/libraries")).unwrap();
    std::fs::create_dir_all(
        repo.paths()
            .instance_root(id)
            .join(".minecraft/versions/1.21.1"),
    )
    .unwrap();
    std::fs::write(
        repo.paths()
            .instance_root(id)
            .join(".minecraft/versions/1.21.1/1.21.1.jar"),
        b"jar content",
    )
    .unwrap();
    std::fs::create_dir_all(temp.path().join("shared/assets/indexes")).unwrap();
    std::fs::write(temp.path().join("shared/assets/indexes/21.json"), b"{}").unwrap();

    let session = graphene_launch::LaunchSession {
        username: "Player".to_string(),
        uuid: "00000000-0000-0000-0000-000000000000".to_string(),
        access_token: graphene_core::SensitiveString::new("secret"),
        user_type: "msa".to_string(),
        client_id: None,
        xuid: None,
    };
    let request = graphene_launch::LaunchRequest::new(id, session);

    // Plan the launch
    let plan = launch_service.plan_with_java(request, java).await.unwrap();
    assert!(plan.state_fingerprint.is_some());

    // Mutate instance state (e.g. modify config or receipt)
    let patch = graphene_instance::InstanceConfigPatch {
        jvm_args: graphene_instance::SettingUpdate::Set(vec!["-Xmx8G".to_string()]),
        ..Default::default()
    };
    service.update_config(id, patch).await.unwrap();

    // Executing the stale plan must fail with InstanceRepairPlanStale
    let err = launch_service.execute(plan).unwrap_err();
    assert_eq!(err.code, graphene_core::ErrorCode::InstanceRepairPlanStale);
}

#[tokio::test]
async fn rename_and_clone_transactions() {
    let (_temp, service) = test_service().await;
    let repo = service.repository();
    let id = InstanceId::new();

    let desc =
        InstanceDescriptor::create(&NewInstanceSpec::with_id(id, "Source Instance").unwrap());
    let receipt = fixture_receipt(id, "1.21.1");
    write_json_atomic(&repo.paths().instance_descriptor_path(id), &desc).unwrap();
    write_json_atomic(&repo.paths().install_receipt_path(id), &receipt).unwrap();
    write_document_atomic(&repo.paths().lockfile_path(id), b"{}").unwrap();
    std::fs::create_dir_all(repo.paths().instance_root(id).join(".minecraft")).unwrap();
    std::fs::write(
        repo.paths()
            .instance_root(id)
            .join(".minecraft/options.txt"),
        b"gamma:1.0",
    )
    .unwrap();

    // 1. Rename
    let renamed = service.rename(id, "Renamed Instance").await.unwrap();
    assert_eq!(renamed.display_name, "Renamed Instance");

    let loaded = service.get(id).await.unwrap();
    assert_eq!(loaded.descriptor.display_name, "Renamed Instance");

    // 2. Clone
    let dest_id = InstanceId::new();
    let clone_req = graphene_instance::CloneRequest::with_id(
        dest_id,
        "Cloned Instance",
        graphene_instance::CloneMode::Full,
    )
    .unwrap();

    let clone_op = service.clone(id, clone_req);
    let cloned_instance = clone_op.await_result().await.unwrap();

    assert_eq!(cloned_instance.descriptor.instance_id, dest_id);
    assert_eq!(cloned_instance.descriptor.display_name, "Cloned Instance");
    assert_eq!(cloned_instance.receipt.instance_id, dest_id);

    // Verify cloned files exist and user data copied
    let dest_root = repo.paths().instance_root(dest_id);
    assert!(dest_root.join("instance.json").exists());
    assert!(dest_root.join(".graphene/install.json").exists());
    assert_eq!(
        std::fs::read(dest_root.join(".minecraft/options.txt")).unwrap(),
        b"gamma:1.0"
    );

    // Verify source is intact
    let source = service.get(id).await.unwrap();
    assert_eq!(source.descriptor.display_name, "Renamed Instance");

    // 3. Delete cloned instance
    let delete_op = service.delete(dest_id, graphene_instance::DeleteOptions::default());
    delete_op.await_result().await.unwrap();

    assert!(!dest_root.exists());
    let get_err = service.get(dest_id).await.unwrap_err();
    assert_eq!(get_err.code, graphene_core::ErrorCode::InstanceNotFound);
}

#[tokio::test]
async fn verification_quick_and_full_reports() {
    let (_temp, service) = test_service().await;
    let repo = service.repository();
    let id = InstanceId::new();

    let desc = InstanceDescriptor::create(&NewInstanceSpec::with_id(id, "Verify Test").unwrap());
    let mut receipt = fixture_receipt(id, "1.21.1");
    let jar_bytes = b"sample jar bytes";
    let jar_path = repo
        .paths()
        .instance_root(id)
        .join(".minecraft/versions/1.21.1/1.21.1.jar");
    std::fs::create_dir_all(jar_path.parent().unwrap()).unwrap();
    std::fs::write(&jar_path, jar_bytes).unwrap();

    let (sha1_exp, sha256_exp) =
        crate::instance_service::verification::stream_compute_hashes(&jar_path).unwrap();
    receipt.client.expected_size = Some(jar_bytes.len() as u64);
    receipt.client.integrity = graphene_core::ArtifactIntegrity::none()
        .with_sha1(sha1_exp)
        .with_sha256(sha256_exp);

    let lockfile = graphene_instance::InstanceLockfile {
        schema_version: graphene_instance::LOCKFILE_SCHEMA_VERSION,
        instance_id: id,
        minecraft_version: "1.21.1".to_string(),
        components: receipt.components.clone(),
        artifacts: vec![graphene_instance::LockedArtifact {
            logical_key: "client".to_string(),
            kind: graphene_core::ArtifactKind::Binary,
            sources: vec![graphene_core::ArtifactSource::new(
                "https://example.invalid/jar",
            )],
            destination: receipt.client.path.clone(),
            scope: graphene_instance::LockedMaterializationScope::InstanceMutable,
            integrity: receipt.client.integrity.clone(),
            expected_size: receipt.client.expected_size,
        }],
        native_extractions: Vec::new(),
        generated_outputs: Vec::new(),
    };

    write_json_atomic(&repo.paths().instance_descriptor_path(id), &desc).unwrap();
    write_json_atomic(&repo.paths().install_receipt_path(id), &receipt).unwrap();
    write_json_atomic(&repo.paths().lockfile_path(id), &lockfile).unwrap();

    // Quick verification: clean
    let report_quick = service
        .verify(id, graphene_instance::VerificationMode::Quick)
        .await_result()
        .await
        .unwrap();
    assert!(report_quick.is_healthy());
    assert_eq!(
        report_quick.repairability,
        graphene_instance::Repairability::Healthy
    );

    // Full verification: clean
    let report_full = service
        .verify(id, graphene_instance::VerificationMode::Full)
        .await_result()
        .await
        .unwrap();
    assert!(report_full.is_healthy());

    // Corrupt jar file by altering bytes
    std::fs::write(&jar_path, b"tampered jar bytes!").unwrap();

    // Quick verify detects size mismatch if size differed
    let report_quick_corrupt = service
        .verify(id, graphene_instance::VerificationMode::Quick)
        .await_result()
        .await
        .unwrap();
    assert!(!report_quick_corrupt.is_healthy());
    assert_eq!(
        report_quick_corrupt.findings[0].code,
        graphene_instance::FindingCode::ManagedFileSizeMismatch
    );

    // Full verify detects hash mismatch
    let report_full_corrupt = service
        .verify(id, graphene_instance::VerificationMode::Full)
        .await_result()
        .await
        .unwrap();
    assert!(!report_full_corrupt.is_healthy());
    assert!(report_full_corrupt.findings.iter().any(|f| f.code
        == graphene_instance::FindingCode::ManagedFileHashMismatch
        || f.code == graphene_instance::FindingCode::ManagedFileSizeMismatch));
}

#[tokio::test]
async fn repair_workflow_planning_execution_and_convergence() {
    let (_temp, service) = test_service().await;
    let repo = service.repository();
    let id = InstanceId::new();

    let desc = InstanceDescriptor::create(&NewInstanceSpec::with_id(id, "Repair Test").unwrap());
    let mut receipt = fixture_receipt(id, "1.21.1");
    let jar_bytes = b"original uncorrupted jar bytes";
    let jar_path = repo
        .paths()
        .instance_root(id)
        .join(".minecraft/versions/1.21.1/1.21.1.jar");
    std::fs::create_dir_all(jar_path.parent().unwrap()).unwrap();
    std::fs::write(&jar_path, jar_bytes).unwrap();

    let (sha1_exp, sha256_exp) =
        crate::instance_service::verification::stream_compute_hashes(&jar_path).unwrap();
    receipt.client.expected_size = Some(jar_bytes.len() as u64);
    receipt.client.integrity = graphene_core::ArtifactIntegrity::none()
        .with_sha1(sha1_exp)
        .with_sha256(sha256_exp);

    let lockfile = graphene_instance::InstanceLockfile {
        schema_version: graphene_instance::LOCKFILE_SCHEMA_VERSION,
        instance_id: id,
        minecraft_version: "1.21.1".to_string(),
        components: receipt.components.clone(),
        artifacts: vec![graphene_instance::LockedArtifact {
            logical_key: "client".to_string(),
            kind: graphene_core::ArtifactKind::Binary,
            sources: vec![graphene_core::ArtifactSource::new(
                "https://example.invalid/jar",
            )],
            destination: receipt.client.path.clone(),
            scope: graphene_instance::LockedMaterializationScope::InstanceMutable,
            integrity: receipt.client.integrity.clone(),
            expected_size: receipt.client.expected_size,
        }],
        native_extractions: Vec::new(),
        generated_outputs: Vec::new(),
    };

    write_json_atomic(&repo.paths().instance_descriptor_path(id), &desc).unwrap();
    write_json_atomic(&repo.paths().install_receipt_path(id), &receipt).unwrap();
    write_json_atomic(&repo.paths().lockfile_path(id), &lockfile).unwrap();

    // Populate local cache so acquire can hit cache offline
    let cache_addr = service
        .context()
        .storage
        .cache_address(&receipt.client.integrity)
        .unwrap();
    let cache_path = cache_addr.path();
    std::fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
    std::fs::write(cache_path, jar_bytes).unwrap();

    // Delete instance jar to induce corruption
    std::fs::remove_file(&jar_path).unwrap();
    assert!(!jar_path.exists());

    // 1. Verify detects missing file
    let verify_report = service
        .verify(id, graphene_instance::VerificationMode::Quick)
        .await_result()
        .await
        .unwrap();
    assert!(!verify_report.is_healthy());

    // 2. Plan repair
    let plan = service
        .plan_repair(
            id,
            graphene_instance::RepairOptions {
                verification_mode: graphene_instance::VerificationMode::Full,
            },
        )
        .await
        .unwrap();
    assert!(!plan.is_noop());
    assert!(!plan.ordered_actions.is_empty());

    // 3. Execute repair
    let repair_result = service
        .execute_repair(plan.clone())
        .await_result()
        .await
        .unwrap();
    assert!(repair_result.post_verify_report.is_healthy());
    assert!(jar_path.exists());
    assert_eq!(std::fs::read(&jar_path).unwrap(), jar_bytes);

    // 4. Convergence: planning repair a second time against clean state is a NO-OP!
    let plan2 = service
        .plan_repair(
            id,
            graphene_instance::RepairOptions {
                verification_mode: graphene_instance::VerificationMode::Full,
            },
        )
        .await
        .unwrap();
    assert!(plan2.is_noop());
    assert!(plan2.ordered_actions.is_empty());

    // 5. Stale plan rejection: execute previous plan after state modification
    let patch = graphene_instance::InstanceConfigPatch {
        jvm_args: graphene_instance::SettingUpdate::Set(vec!["-Xmx16G".to_string()]),
        ..Default::default()
    };
    service.update_config(id, patch).await.unwrap();

    let stale_err = service
        .execute_repair(plan)
        .await_result()
        .await
        .unwrap_err();
    assert_eq!(
        stale_err.code,
        graphene_core::ErrorCode::InstanceRepairPlanStale
    );
}
