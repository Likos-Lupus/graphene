use graphene_core::InstanceId;
use graphene_diagnostics::{
    Confidence, DiagnosticFinding, FindingId, ParsedFinding, RecommendationActionKind,
    correlate_verification, derive_recommendations,
};
use graphene_instance::{
    FindingCode, FindingSeverity, InstanceStateFingerprint, ManagedRelativePath, Repairability,
    VerificationFinding, VerificationMode, VerificationReport,
};

fn instance_id() -> InstanceId {
    "22222222-2222-4222-8222-222222222222"
        .parse()
        .expect("valid id")
}

fn fingerprint() -> InstanceStateFingerprint {
    serde_json::from_str(&format!("\"{}\"", "cd".repeat(32))).expect("valid fingerprint")
}

fn path(value: &str) -> ManagedRelativePath {
    ManagedRelativePath::new(value).expect("valid path")
}

fn report() -> VerificationReport {
    let mut hash_mismatch = VerificationFinding::new(
        FindingCode::ManagedFileHashMismatch,
        FindingSeverity::Error,
        "hash mismatch",
        true,
    )
    .with_path(path(".minecraft/libraries/a.jar"));
    hash_mismatch.logical_item = Some("org.example:a:1.0".to_owned());

    let size_mismatch = VerificationFinding::new(
        FindingCode::ManagedFileSizeMismatch,
        FindingSeverity::Error,
        "size mismatch",
        true,
    )
    .with_path(path(".minecraft/libraries/b.jar"));

    let symlink = VerificationFinding::new(
        FindingCode::UnsafeSymlink,
        FindingSeverity::Error,
        "unsafe symlink",
        false,
    )
    .with_path(path(".minecraft/libraries/c.jar"));

    let legacy = VerificationFinding::new(
        FindingCode::LegacyInstance,
        FindingSeverity::Warning,
        "legacy instance",
        true,
    );

    VerificationReport {
        instance_id: instance_id(),
        mode: VerificationMode::Full,
        state_fingerprint: fingerprint(),
        repairability: Repairability::RepairableWithNetwork,
        findings: vec![hash_mismatch, size_mismatch, symlink, legacy],
        summary: "4 findings".to_owned(),
    }
}

fn to_findings(parsed: &[ParsedFinding]) -> Vec<DiagnosticFinding> {
    parsed
        .iter()
        .enumerate()
        .map(|(index, finding)| {
            DiagnosticFinding::new(
                FindingId::new(u32::try_from(index).expect("small index")),
                finding.diagnostic.clone(),
                finding.confidence,
            )
        })
        .collect()
}

#[test]
fn verification_findings_map_to_stable_codes_and_confidence() {
    let findings = correlate_verification(&report());
    let codes: Vec<&str> = findings
        .iter()
        .map(|finding| finding.diagnostic.code.as_str())
        .collect();

    assert!(codes.contains(&"DIAGNOSTIC_MANAGED_FILE_HASH_MISMATCH"));
    assert!(codes.contains(&"DIAGNOSTIC_MANAGED_FILE_SIZE_MISMATCH"));
    assert!(codes.contains(&"DIAGNOSTIC_UNSAFE_SYMLINK"));
    assert!(codes.contains(&"DIAGNOSTIC_LEGACY_DESIRED_STATE"));

    for finding in &findings {
        assert_eq!(finding.confidence, Confidence::Confirmed);
    }
}

#[test]
fn repairability_evidence_is_preserved_without_storage_internals() {
    let findings = correlate_verification(&report());
    let symlink = findings
        .iter()
        .find(|finding| finding.diagnostic.code.as_str() == "DIAGNOSTIC_UNSAFE_SYMLINK")
        .expect("symlink finding");
    let evidence = &symlink.evidence[0];

    assert_eq!(
        evidence.fields.get("repairable").map(String::as_str),
        Some("false")
    );
    assert_eq!(
        evidence.fields.get("verification_code").map(String::as_str),
        Some("UnsafeSymlink")
    );
    assert_eq!(
        evidence.path.as_ref().map(ManagedRelativePath::as_str),
        Some(".minecraft/libraries/c.jar")
    );

    let hash = findings
        .iter()
        .find(|finding| finding.diagnostic.code.as_str() == "DIAGNOSTIC_MANAGED_FILE_HASH_MISMATCH")
        .expect("hash finding");

    assert_eq!(
        hash.evidence[0]
            .fields
            .get("logical_item")
            .map(String::as_str),
        Some("org.example:a:1.0")
    );
}

#[test]
fn repair_recommendation_converges_and_deduplicates() {
    let parsed = correlate_verification(&report());
    let findings = to_findings(&parsed);
    let recommendations = derive_recommendations(&findings);

    let repair = recommendations
        .iter()
        .find(|recommendation| {
            recommendation.action == RecommendationActionKind::PlanInstanceRepair
        })
        .expect("repair recommendation");
    // hash mismatch, size mismatch, symlink, legacy all map to repair.
    assert_eq!(repair.findings.len(), 4);

    let collect = recommendations
        .iter()
        .find(|recommendation| {
            recommendation.action == RecommendationActionKind::CollectAdditionalEvidence
        })
        .expect("collect recommendation");
    assert_eq!(collect.findings.len(), 1);

    // Deterministic: deriving again yields the same recommendation set.
    let again = derive_recommendations(&findings);
    assert_eq!(recommendations, again);
}

#[test]
fn healthy_verification_produces_no_findings_or_recommendations() {
    let healthy = VerificationReport {
        instance_id: instance_id(),
        mode: VerificationMode::Quick,
        state_fingerprint: fingerprint(),
        repairability: Repairability::Healthy,
        findings: Vec::new(),
        summary: "healthy".to_owned(),
    };
    let parsed = correlate_verification(&healthy);

    assert!(parsed.is_empty());
    assert!(derive_recommendations(&[]).is_empty());
}
