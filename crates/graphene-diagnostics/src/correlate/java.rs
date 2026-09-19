use crate::model::{Confidence, EvidenceSourceKind, JavaEvidence, codes};
use crate::parser::{ParsedEvidence, ParsedFinding};
use graphene_core::{Diagnostic, DiagnosticCode, DiagnosticParameters, DiagnosticSeverity};

fn java_fields(evidence: &JavaEvidence) -> DiagnosticParameters {
    let mut fields = DiagnosticParameters::new();
    if let Some(requirement) = &evidence.requirement {
        fields.insert(
            "required_major".to_owned(),
            requirement.major_version.to_string(),
        );
        if let Some(hint) = &requirement.component_hint {
            fields.insert("component_hint".to_owned(), hint.clone());
        }
    }

    if let Some(selected) = &evidence.selected {
        fields.insert(
            "selected_major".to_owned(),
            selected.major_version.to_string(),
        );
        fields.insert("selected_vendor".to_owned(), selected.vendor.clone());
        fields.insert(
            "selected_architecture".to_owned(),
            selected.architecture.clone(),
        );
    }

    if let Some(compatibility) = &evidence.compatibility {
        fields.insert(
            "compatibility_code".to_owned(),
            compatibility.code.as_str().to_owned(),
        );
    }

    fields.insert("probe_failed".to_owned(), evidence.probe_failed.to_string());
    fields
}

/// Surfaces service-normalized Java compatibility/probe evidence without re-deriving it.
#[must_use]
pub fn correlate_java(evidence: &JavaEvidence) -> Vec<ParsedFinding> {
    let mut findings = Vec::new();
    if evidence.probe_failed {
        findings.push(ParsedFinding {
            diagnostic: Diagnostic {
                code: DiagnosticCode::new(codes::JAVA_PROBE_FAILED),
                severity: DiagnosticSeverity::Warning,
                parameters: DiagnosticParameters::new(),
            },
            confidence: Confidence::Strong,
            evidence: vec![ParsedEvidence::structured(
                EvidenceSourceKind::JavaEvidence,
                None,
                java_fields(evidence),
            )],
        });
    }

    if let Some(compatibility) = &evidence.compatibility
        && compatibility.code.as_str() != "JAVA_COMPATIBLE"
    {
        findings.push(ParsedFinding {
            diagnostic: compatibility.clone(),
            confidence: Confidence::Strong,
            evidence: vec![ParsedEvidence::structured(
                EvidenceSourceKind::JavaEvidence,
                None,
                java_fields(evidence),
            )],
        });
    }

    findings
}

/// Corroborates class-version and JVM-fatal findings with the actually selected runtime evidence.
#[must_use]
pub fn enrich_with_java_evidence(
    findings: &[ParsedFinding],
    evidence: &JavaEvidence,
) -> Vec<ParsedFinding> {
    if evidence.selected.is_none() {
        return findings.to_vec();
    }

    findings
        .iter()
        .cloned()
        .map(|mut finding| {
            let code = finding.diagnostic.code.as_str();
            if code == codes::JAVA_CLASS_VERSION_MISMATCH || code == codes::JVM_FATAL_ERROR {
                finding.evidence.push(ParsedEvidence::structured(
                    EvidenceSourceKind::JavaEvidence,
                    None,
                    java_fields(evidence),
                ));
            }
            finding
        })
        .collect()
}
