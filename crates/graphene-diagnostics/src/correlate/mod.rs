//! Deterministic correlation, deduplication, and recommendation derivation over normalized input.

mod assembler;
mod content;
mod java;
mod recommendations;
mod verification;

use crate::model::{DiagnosticEvidence, DiagnosticFinding, DiagnosticTruncation};
use crate::parser::ParsedFinding;

pub use content::correlate_content;
pub use java::{correlate_java, enrich_with_java_evidence};
pub use recommendations::derive_recommendations;
pub use verification::correlate_verification;

/// Assigns report-local identifiers, deduplicates evidence, and enforces retention ceilings.
#[must_use]
pub fn assemble_findings(
    parsed: &[ParsedFinding],
) -> (
    Vec<DiagnosticFinding>,
    Vec<DiagnosticEvidence>,
    DiagnosticTruncation,
) {
    let mut assembler = assembler::Assembler::new();
    assembler.ingest_many(parsed);
    assembler.into_parts()
}
