pub(crate) mod bounds;
pub(crate) mod codes;
mod evidence;
mod finding;
mod recommendation;
mod report;
mod request;

pub use bounds::{
    DIAGNOSTIC_REPORT_SCHEMA_VERSION, MAX_AGGREGATE_BYTES, MAX_CRASH_REPORTS,
    MAX_EVIDENCE_PER_REPORT, MAX_EXCERPT_BYTES, MAX_FINDINGS_PER_REPORT, MAX_HS_ERR_REPORTS,
    MAX_LINE_BYTES, MAX_RECOMMENDATIONS_PER_REPORT, MAX_SOURCE_BYTES, MAX_SOURCES_PER_REPORT,
};
pub use evidence::{DiagnosticEvidence, EvidenceId, EvidenceSourceKind};
pub use finding::{Confidence, DiagnosticFinding, FindingId};
pub use recommendation::{DiagnosticRecommendation, RecommendationActionKind};
pub use report::{
    DiagnosticCompleteness, DiagnosticReport, DiagnosticSourceSummary, DiagnosticTruncation,
};
pub use request::{
    ContentEnvironment, DiagnosticMode, DiagnosticRequest, DiagnosticSourcePolicy,
    DiagnosticVerificationPolicy, JavaEvidence, JavaRequirementSnapshot, JavaRuntimeSnapshot,
    ProcessExitEvidence,
};
