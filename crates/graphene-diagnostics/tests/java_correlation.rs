use graphene_core::{Diagnostic, DiagnosticCode, DiagnosticParameters, DiagnosticSeverity};
use graphene_diagnostics::{
    Confidence, DiagnosticFinding, EvidenceSourceKind, FindingId, JavaEvidence,
    JavaRequirementSnapshot, JavaRuntimeSnapshot, ParsedEvidence, ParsedFinding,
    RecommendationActionKind, correlate_java, derive_recommendations, enrich_with_java_evidence,
};

fn compatibility(code: &str, severity: DiagnosticSeverity) -> Diagnostic {
    let mut parameters = DiagnosticParameters::new();
    parameters.insert("required_major".to_owned(), "21".to_owned());
    Diagnostic {
        code: DiagnosticCode::new(code),
        severity,
        parameters,
    }
}

fn evidence_with(compatibility: Option<Diagnostic>, probe_failed: bool) -> JavaEvidence {
    JavaEvidence {
        requirement: Some(JavaRequirementSnapshot {
            major_version: 21,
            component_hint: Some("net.fabricmc:fabric-loader".to_owned()),
        }),
        selected: Some(JavaRuntimeSnapshot {
            major_version: 17,
            vendor: "adoptium".to_owned(),
            architecture: "x86_64".to_owned(),
            version: "17.0.9".to_owned(),
        }),
        compatibility,
        probe_failed,
    }
}

#[test]
fn too_old_java_surfaces_exact_compatibility_contract() {
    let evidence = evidence_with(
        Some(compatibility(
            "JAVA_VERSION_TOO_OLD",
            DiagnosticSeverity::Warning,
        )),
        false,
    );

    let findings = correlate_java(&evidence);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].diagnostic.code.as_str(), "JAVA_VERSION_TOO_OLD");
    assert_eq!(findings[0].confidence, Confidence::Strong);

    let fields = &findings[0].evidence[0].fields;

    assert_eq!(fields.get("required_major").map(String::as_str), Some("21"));
    assert_eq!(fields.get("selected_major").map(String::as_str), Some("17"));
    assert_eq!(
        fields.get("compatibility_code").map(String::as_str),
        Some("JAVA_VERSION_TOO_OLD")
    );
    assert_eq!(
        findings[0].evidence[0].source,
        EvidenceSourceKind::JavaEvidence
    );
}

#[test]
fn compatible_java_produces_no_finding() {
    let evidence = evidence_with(
        Some(compatibility("JAVA_COMPATIBLE", DiagnosticSeverity::Info)),
        false,
    );
    assert!(correlate_java(&evidence).is_empty());
}

#[test]
fn probe_failure_produces_finding() {
    let evidence = JavaEvidence {
        requirement: None,
        selected: None,
        compatibility: None,
        probe_failed: true,
    };
    let findings = correlate_java(&evidence);
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].diagnostic.code.as_str(),
        "DIAGNOSTIC_JAVA_PROBE_FAILED"
    );
}

#[test]
fn class_version_mismatch_is_corroborated_by_selected_runtime() {
    let parsed = vec![ParsedFinding {
        diagnostic: Diagnostic {
            code: DiagnosticCode::new("DIAGNOSTIC_JAVA_CLASS_VERSION_MISMATCH"),
            severity: DiagnosticSeverity::Error,
            parameters: DiagnosticParameters::new(),
        },
        confidence: Confidence::Strong,
        evidence: vec![ParsedEvidence::structured(
            EvidenceSourceKind::LogFile,
            None,
            DiagnosticParameters::new(),
        )],
    }];
    let evidence = evidence_with(
        Some(compatibility(
            "JAVA_VERSION_TOO_OLD",
            DiagnosticSeverity::Warning,
        )),
        false,
    );
    let enriched = enrich_with_java_evidence(&parsed, &evidence);

    assert_eq!(
        enriched[0].diagnostic.code.as_str(),
        "DIAGNOSTIC_JAVA_CLASS_VERSION_MISMATCH"
    );
    assert_eq!(enriched[0].evidence.len(), 2);
    assert_eq!(
        enriched[0].evidence[1].source,
        EvidenceSourceKind::JavaEvidence
    );

    let without_runtime = JavaEvidence {
        requirement: Some(JavaRequirementSnapshot {
            major_version: 21,
            component_hint: None,
        }),
        selected: None,
        compatibility: None,
        probe_failed: false,
    };
    let untouched = enrich_with_java_evidence(&parsed, &without_runtime);

    assert_eq!(untouched[0].evidence.len(), 1);
}

#[test]
fn java_incompatibility_maps_to_select_compatible_java() {
    let parsed = correlate_java(&evidence_with(
        Some(compatibility(
            "JAVA_VERSION_TOO_OLD",
            DiagnosticSeverity::Warning,
        )),
        false,
    ));

    let findings: Vec<DiagnosticFinding> = parsed
        .iter()
        .enumerate()
        .map(|(index, finding)| {
            DiagnosticFinding::new(
                FindingId::new(u32::try_from(index).expect("small")),
                finding.diagnostic.clone(),
                finding.confidence,
            )
        })
        .collect();

    let recommendations = derive_recommendations(&findings);

    assert!(
        recommendations
            .iter()
            .any(|recommendation| recommendation.action
                == RecommendationActionKind::SelectCompatibleJava)
    );

    assert!(!recommendations.iter().any(
        |recommendation| recommendation.action == RecommendationActionKind::PlanInstanceRepair
    ));
}
