use graphene_content::{
    ContentInventoryFingerprint, DependencyRelation, EnvironmentSupport, LocalContentFile,
    LocalContentInventory, LocalFileStatus, LocalModDependency, ModMetadataSource,
    NormalizedModDescriptor,
};
use graphene_diagnostics::{
    Confidence, ContentEnvironment, RecommendationActionKind, correlate_content,
    derive_recommendations,
};
use graphene_instance::ManagedRelativePath;

fn path(value: &str) -> ManagedRelativePath {
    ManagedRelativePath::new(value).expect("valid path")
}

fn fingerprint() -> ContentInventoryFingerprint {
    serde_json::from_str(&format!("\"{}\"", "ef".repeat(32))).expect("valid fingerprint")
}

fn dependency(
    mod_id: &str,
    relation: DependencyRelation,
    range: Option<&str>,
) -> LocalModDependency {
    LocalModDependency {
        mod_id: mod_id.to_owned(),
        version_range: range.map(str::to_owned),
        relation,
    }
}

fn descriptor(
    mod_id: &str,
    source: ModMetadataSource,
    dependencies: Vec<LocalModDependency>,
) -> NormalizedModDescriptor {
    let mut descriptor = NormalizedModDescriptor::new(mod_id, mod_id, "1.0.0", source);
    descriptor.dependencies = dependencies;
    descriptor
}

fn mod_file(relative: &str, descriptors: Vec<NormalizedModDescriptor>) -> LocalContentFile {
    LocalContentFile {
        relative_path: path(relative),
        filename: relative.rsplit('/').next().unwrap_or(relative).to_owned(),
        enabled: true,
        size: 1024,
        sha1: None,
        sha256: None,
        murmur2: None,
        descriptors,
        status: LocalFileStatus::UnmanagedKnownMetadata,
        managed_entry_id: None,
        remote_file_ref: None,
        diagnostics: Vec::new(),
    }
}

fn inventory(files: Vec<LocalContentFile>) -> LocalContentInventory {
    LocalContentInventory {
        files,
        duplicate_mod_ids: Vec::new(),
        fingerprint: fingerprint(),
    }
}

#[test]
fn duplicate_logical_mod_ids_are_confirmed_local_conflicts() {
    let mut inventory = inventory(vec![
        mod_file(
            "mods/a.jar",
            vec![descriptor("dupe", ModMetadataSource::Fabric, vec![])],
        ),
        mod_file(
            "mods/b.jar",
            vec![descriptor("dupe", ModMetadataSource::Fabric, vec![])],
        ),
    ]);

    inventory.duplicate_mod_ids = vec![graphene_content::DuplicateModFinding {
        mod_id: "dupe".to_owned(),
        conflicting_files: vec![path("mods/a.jar"), path("mods/b.jar")],
    }];

    let findings = correlate_content(&inventory, None);
    let duplicate = findings
        .iter()
        .find(|finding| finding.diagnostic.code.as_str() == "DIAGNOSTIC_DUPLICATE_MOD_ID")
        .expect("duplicate finding");
    assert_eq!(duplicate.confidence, Confidence::Confirmed);
    assert_eq!(duplicate.evidence.len(), 2);
}

#[test]
fn malformed_archive_is_reported() {
    let mut malformed = mod_file("mods/broken.jar", Vec::new());

    malformed.status = LocalFileStatus::Invalid;
    malformed.diagnostics = vec!["failed inspecting archive metadata".to_owned()];

    let findings = correlate_content(&inventory(vec![malformed]), None);

    assert!(
        findings
            .iter()
            .any(|finding| finding.diagnostic.code.as_str() == "DIAGNOSTIC_MALFORMED_MOD_ARCHIVE")
    );
}

#[test]
fn missing_required_dependency_is_detected_but_builtins_and_present_mods_are_not() {
    let files = vec![
        mod_file(
            "mods/a.jar",
            vec![descriptor(
                "a",
                ModMetadataSource::Fabric,
                vec![
                    dependency("missingmod", DependencyRelation::Required, Some("[1.0,)")),
                    dependency("minecraft", DependencyRelation::Required, None),
                    dependency("present", DependencyRelation::Required, None),
                ],
            )],
        ),
        mod_file(
            "mods/present.jar",
            vec![descriptor("present", ModMetadataSource::Fabric, vec![])],
        ),
    ];

    let findings = correlate_content(&inventory(files), None);
    let missing: Vec<_> = findings
        .iter()
        .filter(|finding| {
            finding.diagnostic.code.as_str() == "DIAGNOSTIC_MISSING_REQUIRED_DEPENDENCY"
        })
        .collect();

    assert_eq!(missing.len(), 1);
    assert_eq!(
        missing[0]
            .diagnostic
            .parameters
            .get("dependency")
            .map(String::as_str),
        Some("missingmod")
    );

    // Raw ambiguous constraint is preserved as evidence rather than evaluated.
    assert_eq!(
        missing[0].evidence[0]
            .fields
            .get("version_range")
            .map(String::as_str),
        Some("[1.0,)")
    );
}

#[test]
fn declared_incompatibility_between_present_mods_is_confirmed() {
    let files = vec![
        mod_file(
            "mods/a.jar",
            vec![descriptor(
                "a",
                ModMetadataSource::Fabric,
                vec![dependency("b", DependencyRelation::Incompatible, None)],
            )],
        ),
        mod_file(
            "mods/b.jar",
            vec![descriptor("b", ModMetadataSource::Fabric, vec![])],
        ),
    ];

    let findings = correlate_content(&inventory(files), None);
    let conflict = findings
        .iter()
        .find(|finding| finding.diagnostic.code.as_str() == "DIAGNOSTIC_CONTENT_INCOMPATIBILITY")
        .expect("conflict finding");
    assert_eq!(conflict.confidence, Confidence::Confirmed);
}

#[test]
fn provable_loader_mismatch_is_detected() {
    let files = vec![mod_file(
        "mods/fabric_mod.jar",
        vec![descriptor("fabricmod", ModMetadataSource::Fabric, vec![])],
    )];

    let environment = ContentEnvironment {
        loader: Some("forge".to_owned()),
        server: false,
    };

    let findings = correlate_content(&inventory(files), Some(&environment));

    assert!(findings.iter().any(
        |finding| finding.diagnostic.code.as_str() == "DIAGNOSTIC_LOADER_ENVIRONMENT_MISMATCH"
    ));

    let matching = ContentEnvironment {
        loader: Some("fabric".to_owned()),
        server: false,
    };

    let files = vec![mod_file(
        "mods/fabric_mod.jar",
        vec![descriptor("fabricmod", ModMetadataSource::Fabric, vec![])],
    )];

    assert!(correlate_content(&inventory(files), Some(&matching)).is_empty());
}

#[test]
fn server_only_mod_on_client_is_detected() {
    let mut module = descriptor("servermod", ModMetadataSource::NeoForge, vec![]);
    module.environment = EnvironmentSupport::ServerOnly;

    let findings = correlate_content(
        &inventory(vec![mod_file("mods/server.jar", vec![module])]),
        Some(&ContentEnvironment {
            loader: Some("neoforge".to_owned()),
            server: false,
        }),
    );

    assert!(findings.iter().any(
        |finding| finding.diagnostic.code.as_str() == "DIAGNOSTIC_LOADER_ENVIRONMENT_MISMATCH"
    ));
}

#[test]
fn local_conflicts_recommend_review_not_automatic_mutation() {
    let mut inventory = inventory(vec![
        mod_file(
            "mods/a.jar",
            vec![descriptor("dupe", ModMetadataSource::Fabric, vec![])],
        ),
        mod_file(
            "mods/b.jar",
            vec![descriptor("dupe", ModMetadataSource::Fabric, vec![])],
        ),
    ]);

    inventory.duplicate_mod_ids = vec![graphene_content::DuplicateModFinding {
        mod_id: "dupe".to_owned(),
        conflicting_files: vec![path("mods/a.jar")],
    }];

    let parsed = correlate_content(&inventory, None);
    let findings: Vec<_> = parsed
        .iter()
        .enumerate()
        .map(|(index, finding)| {
            graphene_diagnostics::DiagnosticFinding::new(
                graphene_diagnostics::FindingId::new(u32::try_from(index).expect("small")),
                finding.diagnostic.clone(),
                finding.confidence,
            )
        })
        .collect();
    let recommendations = derive_recommendations(&findings);

    assert_eq!(recommendations.len(), 1);
    assert_eq!(
        recommendations[0].action,
        RecommendationActionKind::ReviewContentConflict
    );
    assert!(!recommendations.iter().any(
        |recommendation| recommendation.action == RecommendationActionKind::PlanInstanceRepair
    ));
}
