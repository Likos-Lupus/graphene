use super::ParsedFinding;
use crate::model::{Confidence, ProcessExitEvidence, codes};
use graphene_core::{Diagnostic, DiagnosticCode, DiagnosticParameters, DiagnosticSeverity};

/// Produces the fallback evidence-only finding for a non-zero, non-killed process exit.
#[must_use]
pub fn process_exit_finding(exit: &ProcessExitEvidence) -> Option<ParsedFinding> {
    if exit.success || exit.killed {
        return None;
    }
    let mut parameters = DiagnosticParameters::new();
    if let Some(code) = exit.exit_code {
        parameters.insert("exit_code".to_owned(), code.to_string());
    }
    Some(ParsedFinding {
        diagnostic: Diagnostic {
            code: DiagnosticCode::new(codes::PROCESS_EXIT_NONZERO),
            severity: DiagnosticSeverity::Warning,
            parameters,
        },
        confidence: Confidence::Heuristic,
        evidence: Vec::new(),
    })
}

/// Produces the explicit "no explainable cause" finding instead of a fabricated one.
#[must_use]
pub fn cause_undetermined_finding() -> ParsedFinding {
    ParsedFinding {
        diagnostic: Diagnostic {
            code: DiagnosticCode::new(codes::CAUSE_UNDETERMINED),
            severity: DiagnosticSeverity::Warning,
            parameters: DiagnosticParameters::new(),
        },
        confidence: Confidence::Heuristic,
        evidence: Vec::new(),
    }
}
