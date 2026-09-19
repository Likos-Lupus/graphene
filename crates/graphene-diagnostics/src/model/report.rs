use super::bounds::{
    DIAGNOSTIC_REPORT_SCHEMA_VERSION, MAX_EVIDENCE_PER_REPORT, MAX_FINDINGS_PER_REPORT,
    MAX_RECOMMENDATIONS_PER_REPORT, MAX_SOURCES_PER_REPORT,
};
use super::evidence::{DiagnosticEvidence, EvidenceSourceKind};
use super::finding::DiagnosticFinding;
use super::recommendation::{DiagnosticRecommendation, normalize_recommendations};
use super::request::DiagnosticMode;
use crate::error::report_invalid;
use graphene_content::ContentInventoryFingerprint;
use graphene_core::{InstanceId, Result};
use graphene_instance::{InstanceStateFingerprint, ManagedRelativePath};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Whether a report reflects a complete, partial, or demonstrably changed snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiagnosticCompleteness {
    /// All requested evidence was collected from a stable snapshot.
    Complete,
    /// Some requested evidence was unavailable, oversized, or truncated.
    Partial,
    /// Instance state changed during collection; the report is not authoritative.
    Stale,
}

/// Counts of records dropped because a hard ceiling was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct DiagnosticTruncation {
    pub findings_omitted: usize,
    pub evidence_omitted: usize,
    pub sources_omitted: usize,
}

/// Bounded summary of one inspected source, whether or not it produced findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticSourceSummary {
    pub source: EvidenceSourceKind,
    pub path: Option<ManagedRelativePath>,
    pub bytes_inspected: u64,
    pub truncated: bool,
    pub note: Option<String>,
}

impl DiagnosticSourceSummary {
    /// Creates a source summary.
    #[must_use]
    pub fn new(source: EvidenceSourceKind, bytes_inspected: u64, truncated: bool) -> Self {
        Self {
            source,
            path: None,
            bytes_inspected,
            truncated,
            note: None,
        }
    }

    /// Attaches a managed-relative source path.
    #[must_use]
    pub fn with_path(mut self, path: ManagedRelativePath) -> Self {
        self.path = Some(path);
        self
    }

    /// Attaches a bounded redacted note.
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

/// Complete, presentation-independent diagnostic report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub schema_version: u32,
    pub instance_id: InstanceId,
    pub mode: DiagnosticMode,
    pub state_fingerprint: InstanceStateFingerprint,
    pub content_fingerprint: Option<ContentInventoryFingerprint>,
    pub completeness: DiagnosticCompleteness,
    pub findings: Vec<DiagnosticFinding>,
    pub recommendations: Vec<DiagnosticRecommendation>,
    pub evidence: Vec<DiagnosticEvidence>,
    pub sources: Vec<DiagnosticSourceSummary>,
    pub truncation: DiagnosticTruncation,
}

impl DiagnosticReport {
    /// Creates an empty report for one stable snapshot.
    #[must_use]
    pub fn new(
        instance_id: InstanceId,
        mode: DiagnosticMode,
        state_fingerprint: InstanceStateFingerprint,
    ) -> Self {
        Self {
            schema_version: DIAGNOSTIC_REPORT_SCHEMA_VERSION,
            instance_id,
            mode,
            state_fingerprint,
            content_fingerprint: None,
            completeness: DiagnosticCompleteness::Complete,
            findings: Vec::new(),
            recommendations: Vec::new(),
            evidence: Vec::new(),
            sources: Vec::new(),
            truncation: DiagnosticTruncation::default(),
        }
    }

    /// Returns whether the report contains any error or critical finding.
    #[must_use]
    pub fn has_critical(&self) -> bool {
        use graphene_core::DiagnosticSeverity;
        self.findings.iter().any(|finding| {
            matches!(
                finding.diagnostic.severity,
                DiagnosticSeverity::Error | DiagnosticSeverity::Critical
            )
        })
    }

    /// Sorts findings, evidence, sources, and recommendations into a deterministic order and
    /// removes exact duplicate recommendations.
    pub fn normalize(&mut self) {
        self.evidence.sort_by_key(|evidence| evidence.id.value());
        self.evidence.dedup_by_key(|evidence| evidence.id.value());
        self.findings.sort_by_key(|finding| finding.id.value());

        for finding in &mut self.findings {
            finding.evidence.sort_by_key(|id| id.value());
            finding.evidence.dedup();
        }

        normalize_recommendations(&mut self.recommendations);
        self.sources.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| left.bytes_inspected.cmp(&right.bytes_inspected))
        });
        self.sources.dedup();
    }

    /// Validates schema, retention ceilings, identifier uniqueness, and reference integrity.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != DIAGNOSTIC_REPORT_SCHEMA_VERSION {
            return Err(report_invalid(
                "diagnostic report schema version is unsupported",
            ));
        }

        if self.findings.len() > MAX_FINDINGS_PER_REPORT {
            return Err(report_invalid(
                "diagnostic report exceeds the finding ceiling",
            ));
        }

        if self.evidence.len() > MAX_EVIDENCE_PER_REPORT {
            return Err(report_invalid(
                "diagnostic report exceeds the evidence ceiling",
            ));
        }

        if self.recommendations.len() > MAX_RECOMMENDATIONS_PER_REPORT {
            return Err(report_invalid(
                "diagnostic report exceeds the recommendation ceiling",
            ));
        }

        if self.sources.len() > MAX_SOURCES_PER_REPORT {
            return Err(report_invalid(
                "diagnostic report exceeds the source ceiling",
            ));
        }

        let mut evidence_ids = BTreeSet::new();
        for evidence in &self.evidence {
            if !evidence_ids.insert(evidence.id) {
                return Err(report_invalid(
                    "diagnostic evidence identifiers must be unique",
                ));
            }
        }

        let mut finding_ids = BTreeSet::new();
        for finding in &self.findings {
            if !finding_ids.insert(finding.id) {
                return Err(report_invalid(
                    "diagnostic finding identifiers must be unique",
                ));
            }

            for reference in &finding.evidence {
                if !evidence_ids.contains(reference) {
                    return Err(report_invalid(
                        "diagnostic finding references unknown evidence",
                    ));
                }
            }
        }

        for recommendation in &self.recommendations {
            for reference in &recommendation.findings {
                if !finding_ids.contains(reference) {
                    return Err(report_invalid(
                        "diagnostic recommendation references unknown finding",
                    ));
                }
            }
        }

        Ok(())
    }
}
