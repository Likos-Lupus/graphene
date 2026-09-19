//! Bounded, offline-first diagnostic normalization, evidence correlation, and recommendation.
//!
//! This crate owns provider-neutral diagnostic models and pure analysis over already-collected,
//! bounded snapshots. It performs no filesystem access, instance leasing, Java probing, content
//! scanning, network access, or mutation; orchestration stays in `graphene-service`.

mod analysis;
mod correlate;
mod error;
mod model;
mod parser;

pub use analysis::{
    BoundedLine, DATA_ROOT_PLACEHOLDER, HOME_PLACEHOLDER, LineScanner, RedactedExcerpt,
    RedactionContext, Redactor, SECRET_PLACEHOLDER, bounded_lines, decode_lossy, truncate_head,
    truncate_tail,
};
pub use correlate::{
    assemble_findings, correlate_content, correlate_java, correlate_verification,
    derive_recommendations, enrich_with_java_evidence,
};
pub use model::{
    Confidence, ContentEnvironment, DIAGNOSTIC_REPORT_SCHEMA_VERSION, DiagnosticCompleteness,
    DiagnosticEvidence, DiagnosticFinding, DiagnosticMode, DiagnosticRecommendation,
    DiagnosticReport, DiagnosticRequest, DiagnosticSourcePolicy, DiagnosticSourceSummary,
    DiagnosticTruncation, DiagnosticVerificationPolicy, EvidenceId, EvidenceSourceKind, FindingId,
    JavaEvidence, JavaRequirementSnapshot, JavaRuntimeSnapshot, MAX_AGGREGATE_BYTES,
    MAX_CRASH_REPORTS, MAX_EVIDENCE_PER_REPORT, MAX_EXCERPT_BYTES, MAX_FINDINGS_PER_REPORT,
    MAX_HS_ERR_REPORTS, MAX_LINE_BYTES, MAX_RECOMMENDATIONS_PER_REPORT, MAX_SOURCE_BYTES,
    MAX_SOURCES_PER_REPORT, ProcessExitEvidence, RecommendationActionKind,
};
pub use parser::{
    ParseLimits, ParsedEvidence, ParsedFinding, TextSource, cause_undetermined_finding,
    has_severe_finding, parse_sources, process_exit_finding,
};
