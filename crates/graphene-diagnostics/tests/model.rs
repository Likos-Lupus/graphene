use graphene_core::{
    Diagnostic, DiagnosticCode, DiagnosticParameters, DiagnosticSeverity, ErrorCode, InstanceId,
};
use graphene_diagnostics::{
    Confidence, DIAGNOSTIC_REPORT_SCHEMA_VERSION, DiagnosticCompleteness, DiagnosticEvidence,
    DiagnosticFinding, DiagnosticMode, DiagnosticRecommendation, DiagnosticReport,
    DiagnosticRequest, DiagnosticSourcePolicy, DiagnosticVerificationPolicy, EvidenceId,
    EvidenceSourceKind, FindingId, MAX_CRASH_REPORTS, RecommendationActionKind,
};
use graphene_instance::InstanceStateFingerprint;

fn fingerprint() -> InstanceStateFingerprint {
    serde_json::from_str(&format!("\"{}\"", "ab".repeat(32))).expect("valid fingerprint")
}

fn instance_id() -> InstanceId {
    "11111111-1111-4111-8111-111111111111"
        .parse()
        .expect("valid id")
}

fn diagnostic(code: &str, severity: DiagnosticSeverity) -> Diagnostic {
    Diagnostic {
        code: DiagnosticCode::new(code),
        severity,
        parameters: DiagnosticParameters::new(),
    }
}

fn assert_send_sync<T: Send + Sync + 'static>() {}

#[test]
fn public_models_are_host_independent_and_thread_safe() {
    assert_send_sync::<DiagnosticRequest>();
    assert_send_sync::<DiagnosticReport>();
    assert_send_sync::<DiagnosticFinding>();
    assert_send_sync::<DiagnosticRecommendation>();
}

#[test]
fn confidence_ordering_is_heuristic_then_strong_then_confirmed() {
    assert!(Confidence::Heuristic < Confidence::Strong);
    assert!(Confidence::Strong < Confidence::Confirmed);
    assert_eq!(Confidence::default(), Confidence::Heuristic);
}

#[test]
fn confidence_and_mode_serialize_as_stable_upper_snake_tokens() {
    assert_eq!(
        serde_json::to_string(&Confidence::Confirmed).expect("serialize"),
        "\"CONFIRMED\""
    );
    assert_eq!(
        serde_json::to_string(&DiagnosticMode::Crash).expect("serialize"),
        "\"CRASH\""
    );
    assert_eq!(
        serde_json::to_string(&DiagnosticVerificationPolicy::Skip).expect("serialize"),
        "\"SKIP\""
    );
}

#[test]
fn default_source_policy_is_at_the_hard_ceiling_and_valid() {
    let policy = DiagnosticSourcePolicy::default();
    assert_eq!(policy.max_crash_reports, MAX_CRASH_REPORTS);
    policy.validate().expect("default policy must be valid");
}

#[test]
fn source_policy_rejects_unbounded_and_inverted_limits() {
    let too_many = DiagnosticSourcePolicy {
        max_crash_reports: MAX_CRASH_REPORTS + 1,
        ..DiagnosticSourcePolicy::default()
    };
    assert_eq!(
        too_many.validate().expect_err("rejected").code,
        ErrorCode::DiagnosticRequestInvalid
    );

    let inverted = DiagnosticSourcePolicy {
        max_source_bytes: 1024,
        max_total_bytes: 512,
        ..DiagnosticSourcePolicy::default()
    };
    assert!(inverted.validate().is_err());
}

#[test]
fn generated_request_constructors_validate() {
    assert_eq!(DiagnosticRequest::full().mode, DiagnosticMode::Full);
    assert_eq!(
        DiagnosticRequest::full().verification,
        DiagnosticVerificationPolicy::Full
    );

    DiagnosticRequest::preflight().validate().expect("valid");
    DiagnosticRequest::crash().validate().expect("valid");
}

#[test]
fn report_rejects_unknown_evidence_reference() {
    let mut report = DiagnosticReport::new(instance_id(), DiagnosticMode::Full, fingerprint());

    report.evidence.push(DiagnosticEvidence::structured(
        EvidenceId::new(1),
        EvidenceSourceKind::LogFile,
        DiagnosticParameters::new(),
    ));

    report.findings.push(
        DiagnosticFinding::new(
            FindingId::new(1),
            diagnostic("DIAGNOSTIC_CAUSE_UNDETERMINED", DiagnosticSeverity::Warning),
            Confidence::Heuristic,
        )
        .with_evidence([EvidenceId::new(99)]),
    );

    let error = report.validate().expect_err("unknown evidence reference");
    assert_eq!(error.code, ErrorCode::DiagnosticReportInvalid);
}

#[test]
fn report_rejects_unknown_finding_reference_and_duplicate_ids() {
    let mut report = DiagnosticReport::new(instance_id(), DiagnosticMode::Full, fingerprint());
    let recommendation = DiagnosticRecommendation::new(
        DiagnosticCode::new("RECOMMENDED_REPAIR"),
        RecommendationActionKind::PlanInstanceRepair,
        DiagnosticParameters::new(),
    )
    .with_findings([FindingId::new(7)]);

    report.recommendations.push(recommendation);

    assert!(report.validate().is_err());

    report.recommendations.clear();
    report.evidence.push(DiagnosticEvidence::structured(
        EvidenceId::new(1),
        EvidenceSourceKind::LogFile,
        DiagnosticParameters::new(),
    ));
    report.evidence.push(DiagnosticEvidence::structured(
        EvidenceId::new(1),
        EvidenceSourceKind::LogFile,
        DiagnosticParameters::new(),
    ));

    assert!(report.validate().is_err());
}

#[test]
fn normalize_deduplicates_recommendations_and_orders_them_deterministically() {
    let mut report = DiagnosticReport::new(instance_id(), DiagnosticMode::Full, fingerprint());
    let repair = DiagnosticRecommendation::new(
        DiagnosticCode::new("REPAIR"),
        RecommendationActionKind::PlanInstanceRepair,
        DiagnosticParameters::new(),
    );
    let collect = DiagnosticRecommendation::new(
        DiagnosticCode::new("COLLECT"),
        RecommendationActionKind::CollectAdditionalEvidence,
        DiagnosticParameters::new(),
    );

    report.recommendations.push(collect.clone());
    report.recommendations.push(repair.clone());
    report.recommendations.push(collect.clone());
    report.recommendations.push(repair);

    report.normalize();

    assert_eq!(report.recommendations.len(), 2);
    assert_eq!(
        report.recommendations[0].action,
        RecommendationActionKind::PlanInstanceRepair
    );
    assert_eq!(
        report.recommendations[1].action,
        RecommendationActionKind::CollectAdditionalEvidence
    );
}

#[test]
fn report_serializes_and_deserializes_without_losing_shape() {
    let mut report = DiagnosticReport::new(instance_id(), DiagnosticMode::Full, fingerprint());

    report.completeness = DiagnosticCompleteness::Partial;
    report.schema_version = DIAGNOSTIC_REPORT_SCHEMA_VERSION;

    let encoded = serde_json::to_string(&report).expect("serialize report");
    let decoded: DiagnosticReport = serde_json::from_str(&encoded).expect("deserialize report");

    assert_eq!(decoded, report);
}
