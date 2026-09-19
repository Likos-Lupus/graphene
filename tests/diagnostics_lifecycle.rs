use graphene::{
    ArtifactIntegrity, DiagnosticCompleteness, DiagnosticRequest, ErrorCode, Graphene,
    InstallReceipt, InstalledArtifact, InstalledComponent, InstalledComponentKind,
    InstalledJavaRequirement, InstanceDescriptor, InstanceId, InstanceLockfile, InstanceRepository,
    NewInstanceSpec, ProcessExitEvidence, RecommendationActionKind, RepairOptions,
};
use std::path::Path;

fn instance_id() -> InstanceId {
    InstanceId::from_bytes([7; 16])
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
        client: InstalledArtifact {
            path: graphene::ManagedRelativePath::new(format!(
                ".minecraft/versions/{version}/{version}.jar"
            ))
            .expect("valid path"),
            integrity: ArtifactIntegrity::default(),
            expected_size: Some(100),
        },
        libraries: Vec::new(),
        asset_index_id: "21".to_string(),
        asset_index: InstalledArtifact {
            path: graphene::ManagedRelativePath::new("shared/assets/indexes/21.json")
                .expect("valid path"),
            integrity: ArtifactIntegrity::default(),
            expected_size: Some(100),
        },
        assets_root: graphene::ManagedRelativePath::new("shared/assets").expect("valid path"),
        natives_directory: graphene::ManagedRelativePath::new(format!(
            ".graphene/natives/{version}"
        ))
        .expect("valid path"),
        logging_configuration: None,
        logging_argument: None,
        jvm_arguments: Vec::new(),
        game_arguments: Vec::new(),
    }
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
    std::fs::write(path, serde_json::to_vec_pretty(value).expect("serialize")).expect("write");
}

fn ready_lockfile(id: InstanceId, version: &str) -> InstanceLockfile {
    InstanceLockfile {
        schema_version: 3,
        instance_id: id,
        minecraft_version: version.to_string(),
        components: vec![InstalledComponent {
            uid: "net.minecraft".to_string(),
            version: version.to_string(),
            kind: InstalledComponentKind::Minecraft,
            provider: "mojang".to_string(),
            provenance: None,
        }],
        artifacts: Vec::new(),
        native_extractions: Vec::new(),
        generated_outputs: Vec::new(),
        content: Vec::new(),
        pack_origin: None,
    }
}

async fn engine_with_legacy_instance()
-> (tempfile::TempDir, Graphene, InstanceId, InstanceRepository) {
    let temp = tempfile::tempdir().expect("tempdir");
    let engine = Graphene::builder(temp.path())
        .build()
        .await
        .expect("engine");
    let id = instance_id();
    let repository = engine.instances().repository().clone();
    let descriptor =
        InstanceDescriptor::create(&NewInstanceSpec::with_id(id, "Diag").expect("spec"));

    write_json(
        &repository.paths().instance_descriptor_path(id),
        &descriptor,
    );
    write_json(
        &repository.paths().install_receipt_path(id),
        &fixture_receipt(id, "1.21.1"),
    );

    let client = repository
        .paths()
        .minecraft_dir(id)
        .join("versions/1.21.1/1.21.1.jar");
    std::fs::create_dir_all(client.parent().expect("parent")).expect("client dir");
    std::fs::write(&client, vec![0u8; 100]).expect("client file");
    (temp, engine, id, repository)
}

#[tokio::test]
async fn healthy_ready_instance_reports_no_critical_findings() {
    let temp = tempfile::tempdir().expect("tempdir");
    let engine = Graphene::builder(temp.path())
        .build()
        .await
        .expect("engine");
    let id = instance_id();
    let repository = engine.instances().repository().clone();
    let descriptor =
        InstanceDescriptor::create(&NewInstanceSpec::with_id(id, "Diag").expect("spec"));

    write_json(
        &repository.paths().instance_descriptor_path(id),
        &descriptor,
    );
    write_json(
        &repository.paths().install_receipt_path(id),
        &fixture_receipt(id, "1.21.1"),
    );
    write_json(
        &repository.paths().lockfile_path(id),
        &ready_lockfile(id, "1.21.1"),
    );

    let report = engine
        .diagnostics()
        .analyze(id, DiagnosticRequest::preflight())
        .await_result()
        .await
        .expect("analysis");
    report.validate().expect("valid report");
    let codes: Vec<&str> = report
        .findings
        .iter()
        .map(|finding| finding.diagnostic.code.as_str())
        .collect();

    assert!(
        !report.has_critical(),
        "healthy instance must not be critical: {codes:?}"
    );
    assert!(
        !report
            .findings
            .iter()
            .any(|finding| finding.diagnostic.code.as_str() == "DIAGNOSTIC_LEGACY_DESIRED_STATE")
    );
}

#[tokio::test]
async fn legacy_instance_recommends_existing_repair_service() {
    let (_temp, engine, id, _repository) = engine_with_legacy_instance().await;
    let report = engine
        .diagnostics()
        .analyze(id, DiagnosticRequest::preflight())
        .await_result()
        .await
        .expect("analysis");

    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.diagnostic.code.as_str() == "DIAGNOSTIC_LEGACY_DESIRED_STATE")
    );
    assert!(report.recommendations.iter().any(
        |recommendation| recommendation.action == RecommendationActionKind::PlanInstanceRepair
    ));
    assert!(!report.has_critical());

    // The repair recommendation converges on the existing repair service without any new executor.
    let plan = engine
        .instances()
        .plan_repair(id, RepairOptions::default())
        .await
        .expect("repair plan");

    assert_eq!(plan.instance_id, id);
    assert_eq!(plan.base_state_fingerprint, report.state_fingerprint);
}

#[tokio::test]
async fn crash_log_and_process_exit_produce_bounded_report() {
    let (_temp, engine, id, repository) = engine_with_legacy_instance().await;
    let logs = repository.paths().minecraft_dir(id).join("logs");

    std::fs::create_dir_all(&logs).expect("create logs");
    std::fs::write(
        logs.join("latest.log"),
        "java.lang.OutOfMemoryError: Java heap space\n",
    )
    .expect("write log");

    let request = DiagnosticRequest::crash().with_process_exit(ProcessExitEvidence::new(
        Some(1),
        false,
        false,
    ));
    let report = engine
        .diagnostics()
        .analyze(id, request)
        .await_result()
        .await
        .expect("analysis");
    report.validate().expect("valid report");

    let codes: Vec<&str> = report
        .findings
        .iter()
        .map(|finding| finding.diagnostic.code.as_str())
        .collect();

    assert!(codes.contains(&"DIAGNOSTIC_JVM_OUT_OF_MEMORY"));
    assert!(codes.contains(&"DIAGNOSTIC_PROCESS_EXIT_NONZERO"));
    assert!(
        report
            .recommendations
            .iter()
            .any(|recommendation| recommendation.action
                == RecommendationActionKind::ReviewMemoryConfiguration)
    );
}

#[tokio::test]
async fn unrecognized_crash_is_explicitly_undetermined() {
    let (_temp, engine, id, repository) = engine_with_legacy_instance().await;
    let logs = repository.paths().minecraft_dir(id).join("logs");

    std::fs::create_dir_all(&logs).expect("create logs");
    std::fs::write(logs.join("latest.log"), "nothing recognizable here\n").expect("write log");

    let report = engine
        .diagnostics()
        .analyze(id, DiagnosticRequest::crash())
        .await_result()
        .await
        .expect("analysis");

    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.diagnostic.code.as_str() == "DIAGNOSTIC_CAUSE_UNDETERMINED")
    );
}

#[tokio::test]
async fn analysis_is_read_only() {
    let (_temp, engine, id, repository) = engine_with_legacy_instance().await;
    let document_paths = [
        repository.paths().instance_descriptor_path(id),
        repository.paths().install_receipt_path(id),
        repository.paths().config_path(id),
        repository.paths().lockfile_path(id),
    ];
    let before: Vec<Option<Vec<u8>>> = document_paths
        .iter()
        .map(|path| std::fs::read(path).ok())
        .collect();

    let mods = repository.paths().minecraft_dir(id).join("mods");
    std::fs::create_dir_all(&mods).expect("mods dir");
    std::fs::write(mods.join("sample.jar"), b"not a real jar").expect("mod");
    let mods_before = std::fs::read_dir(&mods).expect("read mods").count();

    let report = engine
        .diagnostics()
        .analyze(id, DiagnosticRequest::full())
        .await_result()
        .await
        .expect("analysis");
    report.validate().expect("valid report");

    let after: Vec<Option<Vec<u8>>> = document_paths
        .iter()
        .map(|path| std::fs::read(path).ok())
        .collect();
    assert_eq!(before, after, "diagnosis must not mutate persisted state");
    assert_eq!(
        std::fs::read_dir(&mods).expect("read mods").count(),
        mods_before
    );
}

#[tokio::test]
async fn identical_snapshot_produces_identical_report() {
    let (_temp, engine, id, repository) = engine_with_legacy_instance().await;
    let logs = repository.paths().minecraft_dir(id).join("logs");

    std::fs::create_dir_all(&logs).expect("create logs");
    std::fs::write(logs.join("latest.log"), "java.lang.OutOfMemoryError\n").expect("write log");

    let mut request = DiagnosticRequest::full();
    request.sources.collect_java = false;

    let first = engine
        .diagnostics()
        .analyze(id, request.clone())
        .await_result()
        .await
        .expect("first analysis");
    let second = engine
        .diagnostics()
        .analyze(id, request)
        .await_result()
        .await
        .expect("second analysis");
    assert_eq!(first, second);
    assert_eq!(first.completeness, DiagnosticCompleteness::Complete);
}

#[tokio::test]
async fn operation_handle_supports_cancel_and_await() {
    let (_temp, engine, id, _repository) = engine_with_legacy_instance().await;
    let operation = engine.diagnostics().analyze(id, DiagnosticRequest::full());
    let handle = operation.operation();

    assert!(!handle.id().to_string().is_empty());
    handle.cancel();
    let _ = operation.await_result().await;
    assert!(handle.current_state().is_terminal());
}

#[tokio::test]
async fn diagnostic_report_does_not_bypass_repair_stale_protection() {
    let (_temp, engine, id, repository) = engine_with_legacy_instance().await;
    let report = engine
        .diagnostics()
        .analyze(id, DiagnosticRequest::preflight())
        .await_result()
        .await
        .expect("analysis");

    assert!(report.recommendations.iter().any(
        |recommendation| recommendation.action == RecommendationActionKind::PlanInstanceRepair
    ));

    let plan = engine
        .instances()
        .plan_repair(id, RepairOptions::default())
        .await
        .expect("repair plan");

    // External state change after planning and diagnosis.
    write_json(
        &repository.paths().install_receipt_path(id),
        &fixture_receipt(id, "1.21.2"),
    );

    let error = engine
        .instances()
        .execute_repair(plan)
        .await_result()
        .await
        .expect_err("stale repair plan must be rejected");
    assert_eq!(error.code, ErrorCode::InstanceRepairPlanStale);
}

#[tokio::test]
async fn diagnostic_evidence_never_exposes_secret_sentinels() {
    let (_temp, engine, id, repository) = engine_with_legacy_instance().await;
    let sentinel = "GRAPHENE-E2E-SENTINEL-6b1";
    let logs = repository.paths().minecraft_dir(id).join("logs");

    std::fs::create_dir_all(&logs).expect("create logs");
    std::fs::write(
        logs.join("latest.log"),
        format!(
            "java.lang.OutOfMemoryError: Java heap space\naccess_token={sentinel}\nBearer {sentinel}\n"
        ),
    )
    .expect("write log");

    let report = engine
        .diagnostics()
        .analyze(id, DiagnosticRequest::crash())
        .await_result()
        .await
        .expect("analysis");
    let json = serde_json::to_string(&report).expect("serialize report");

    assert!(!json.contains(sentinel));
    assert!(!format!("{report:?}").contains(sentinel));
}
