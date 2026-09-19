//! Bounded, panic-free text/crash parsing that preserves unknown evidence.

mod exit;
mod rules;

use crate::analysis::redaction::Redactor;
use crate::model::{
    Confidence, EvidenceSourceKind, MAX_EXCERPT_BYTES, MAX_FINDINGS_PER_REPORT, MAX_LINE_BYTES,
};
use graphene_core::{Diagnostic, DiagnosticParameters};
use graphene_instance::ManagedRelativePath;
use serde::{Deserialize, Serialize};

pub use exit::{cause_undetermined_finding, process_exit_finding};
use rules::RULES;

/// One already-collected, decoded, bounded textual source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSource {
    pub source: EvidenceSourceKind,
    pub path: Option<ManagedRelativePath>,
    pub text: String,
    pub truncated: bool,
}

impl TextSource {
    /// Creates a text source.
    #[must_use]
    pub fn new(
        source: EvidenceSourceKind,
        path: Option<ManagedRelativePath>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            source,
            path,
            text: text.into(),
            truncated: false,
        }
    }

    /// Marks the source as truncated during collection.
    #[must_use]
    pub fn truncated(mut self) -> Self {
        self.truncated = true;
        self
    }
}

/// Bounded parser output before report-local identifiers are assigned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedFinding {
    pub diagnostic: Diagnostic,
    pub confidence: Confidence,
    pub evidence: Vec<ParsedEvidence>,
}

/// Bounded, already-redacted parser evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedEvidence {
    pub source: EvidenceSourceKind,
    pub path: Option<ManagedRelativePath>,
    pub line: Option<u64>,
    pub excerpt: Option<String>,
    pub truncated: bool,
    pub fields: DiagnosticParameters,
}

impl ParsedEvidence {
    /// Creates structured-only evidence without a text excerpt.
    #[must_use]
    pub fn structured(
        source: EvidenceSourceKind,
        path: Option<ManagedRelativePath>,
        fields: DiagnosticParameters,
    ) -> Self {
        Self {
            source,
            path,
            line: None,
            excerpt: None,
            truncated: false,
            fields,
        }
    }
}

/// Explicit parser resource limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseLimits {
    pub max_findings: usize,
    pub max_evidence_per_finding: usize,
    pub max_excerpt_bytes: usize,
    pub max_line_bytes: usize,
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_findings: MAX_FINDINGS_PER_REPORT,
            max_evidence_per_finding: 4,
            max_excerpt_bytes: MAX_EXCERPT_BYTES,
            max_line_bytes: MAX_LINE_BYTES,
        }
    }
}

/// Runs the deterministic rule set over bounded text sources.
///
/// Unknown or ambiguous input yields no finding here; callers decide whether to surface an
/// explicit undetermined cause rather than fabricating one.
#[must_use]
pub fn parse_sources(
    sources: &[TextSource],
    redactor: &Redactor,
    limits: &ParseLimits,
) -> Vec<ParsedFinding> {
    let mut findings = Vec::new();
    for rule in RULES {
        if findings.len() >= limits.max_findings {
            break;
        }

        if let Some(finding) = rules::detect_rule(
            rule,
            sources,
            redactor,
            limits.max_excerpt_bytes,
            limits.max_line_bytes,
            limits.max_evidence_per_finding,
        ) {
            findings.push(finding);
        }
    }

    findings
}

/// Returns whether any parsed finding is error- or critical-severity.
#[must_use]
pub fn has_severe_finding(findings: &[ParsedFinding]) -> bool {
    use graphene_core::DiagnosticSeverity;
    findings.iter().any(|finding| {
        matches!(
            finding.diagnostic.severity,
            DiagnosticSeverity::Error | DiagnosticSeverity::Critical
        )
    })
}
