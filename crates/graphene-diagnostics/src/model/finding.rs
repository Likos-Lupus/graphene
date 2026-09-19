use super::evidence::EvidenceId;
use graphene_core::Diagnostic;
use serde::{Deserialize, Serialize};

/// How strongly the collected evidence supports one finding.
///
/// Ordering is intentional: `Heuristic < Strong < Confirmed`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Confidence {
    /// Weak, correlational evidence such as a stack-trace reference.
    #[default]
    Heuristic,
    /// Strong evidence that does not by itself prove the root cause.
    Strong,
    /// Direct, deterministic evidence such as a failed integrity check.
    Confirmed,
}

/// Stable report-local identifier for one diagnostic finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FindingId(u32);

impl FindingId {
    /// Creates a finding identifier.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the numeric identifier.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}

/// A normalized diagnostic finding with explicit confidence and evidence references.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticFinding {
    pub id: FindingId,
    pub diagnostic: Diagnostic,
    pub confidence: Confidence,
    pub evidence: Vec<EvidenceId>,
}

impl DiagnosticFinding {
    /// Creates a finding with no evidence references.
    #[must_use]
    pub fn new(id: FindingId, diagnostic: Diagnostic, confidence: Confidence) -> Self {
        Self {
            id,
            diagnostic,
            confidence,
            evidence: Vec::new(),
        }
    }

    /// Attaches ordered evidence references.
    #[must_use]
    pub fn with_evidence(mut self, evidence: impl IntoIterator<Item = EvidenceId>) -> Self {
        self.evidence = evidence.into_iter().collect();
        self
    }
}
