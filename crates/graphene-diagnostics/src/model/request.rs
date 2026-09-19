use super::bounds::{
    MAX_AGGREGATE_BYTES, MAX_CRASH_REPORTS, MAX_EXCERPT_BYTES, MAX_HS_ERR_REPORTS, MAX_SOURCE_BYTES,
};
use crate::error::request_invalid;
use graphene_core::{Diagnostic, Result};
use serde::{Deserialize, Serialize};

/// Diagnostic analysis mode selected by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiagnosticMode {
    /// Fast structural preflight without full cryptographic verification.
    #[default]
    Preflight,
    /// Focused analysis of a launch failure or crash.
    Crash,
    /// Complete evidence collection and analysis.
    Full,
}

/// Instance verification policy applied during diagnosis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiagnosticVerificationPolicy {
    /// Structural and bounded size checks without hashing large archives.
    #[default]
    Quick,
    /// Full cryptographic integrity verification.
    Full,
    /// Do not run instance verification.
    Skip,
}

/// Which local sources the collector may inspect, with explicit ceilings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticSourcePolicy {
    pub collect_logs: bool,
    pub collect_crash_reports: bool,
    pub collect_hs_err: bool,
    pub scan_content: bool,
    pub collect_java: bool,
    pub max_crash_reports: usize,
    pub max_hs_err_reports: usize,
    pub max_source_bytes: u64,
    pub max_total_bytes: u64,
    pub max_excerpt_bytes: usize,
}

impl Default for DiagnosticSourcePolicy {
    fn default() -> Self {
        Self {
            collect_logs: true,
            collect_crash_reports: true,
            collect_hs_err: true,
            scan_content: true,
            collect_java: true,
            max_crash_reports: MAX_CRASH_REPORTS,
            max_hs_err_reports: MAX_HS_ERR_REPORTS,
            max_source_bytes: MAX_SOURCE_BYTES,
            max_total_bytes: MAX_AGGREGATE_BYTES,
            max_excerpt_bytes: MAX_EXCERPT_BYTES,
        }
    }
}

impl DiagnosticSourcePolicy {
    /// Rejects unbounded or zero limits and values above the hard ceilings.
    pub fn validate(&self) -> Result<()> {
        if self.max_crash_reports > MAX_CRASH_REPORTS {
            return Err(request_invalid(
                "max_crash_reports exceeds the supported ceiling",
            ));
        }

        if self.max_hs_err_reports > MAX_HS_ERR_REPORTS {
            return Err(request_invalid(
                "max_hs_err_reports exceeds the supported ceiling",
            ));
        }

        if self.max_source_bytes == 0 || self.max_source_bytes > MAX_SOURCE_BYTES {
            return Err(request_invalid(
                "max_source_bytes is outside the supported range",
            ));
        }

        if self.max_total_bytes == 0 || self.max_total_bytes > MAX_AGGREGATE_BYTES {
            return Err(request_invalid(
                "max_total_bytes is outside the supported range",
            ));
        }

        if self.max_excerpt_bytes == 0 || self.max_excerpt_bytes > MAX_EXCERPT_BYTES {
            return Err(request_invalid(
                "max_excerpt_bytes is outside the supported range",
            ));
        }

        if self.max_total_bytes < self.max_source_bytes {
            return Err(request_invalid(
                "max_total_bytes must be at least max_source_bytes",
            ));
        }

        Ok(())
    }
}

/// Caller-supplied process exit evidence from an earlier launch attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessExitEvidence {
    pub exit_code: Option<i32>,
    pub success: bool,
    pub killed: bool,
    pub stdout_tail: Option<String>,
    pub stderr_tail: Option<String>,
}

impl ProcessExitEvidence {
    /// Creates exit evidence without captured output tails.
    #[must_use]
    pub fn new(exit_code: Option<i32>, success: bool, killed: bool) -> Self {
        Self {
            exit_code,
            success,
            killed,
            stdout_tail: None,
            stderr_tail: None,
        }
    }

    /// Rejects caller-supplied tails larger than the per-source ceiling.
    pub fn validate(&self) -> Result<()> {
        for tail in [&self.stdout_tail, &self.stderr_tail].into_iter().flatten() {
            if tail.len() as u64 > MAX_SOURCE_BYTES {
                return Err(request_invalid(
                    "process output tail exceeds the source ceiling",
                ));
            }
        }
        Ok(())
    }
}

/// Normalized Java requirement supplied by the service layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaRequirementSnapshot {
    pub major_version: u32,
    pub component_hint: Option<String>,
}

/// Normalized selected Java runtime supplied by the service layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaRuntimeSnapshot {
    pub major_version: u32,
    pub vendor: String,
    pub architecture: String,
    pub version: String,
}

/// Service-normalized Java evidence; the diagnostics crate never re-parses Java versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct JavaEvidence {
    pub requirement: Option<JavaRequirementSnapshot>,
    pub selected: Option<JavaRuntimeSnapshot>,
    pub compatibility: Option<Diagnostic>,
    pub probe_failed: bool,
}

/// Service-normalized instance environment used for provable loader/side mismatch checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ContentEnvironment {
    /// Normalized loader family such as `fabric`, `forge`, or `neoforge`.
    pub loader: Option<String>,
    /// Whether the instance targets a server environment.
    pub server: bool,
}

/// A bounded, presentation-independent diagnostic request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticRequest {
    pub mode: DiagnosticMode,
    pub verification: DiagnosticVerificationPolicy,
    pub sources: DiagnosticSourcePolicy,
    pub process_exit: Option<ProcessExitEvidence>,
}

impl DiagnosticRequest {
    /// Creates a fast, structural preflight request.
    #[must_use]
    pub fn preflight() -> Self {
        Self {
            mode: DiagnosticMode::Preflight,
            verification: DiagnosticVerificationPolicy::Quick,
            sources: DiagnosticSourcePolicy::default(),
            process_exit: None,
        }
    }

    /// Creates a focused crash analysis request.
    #[must_use]
    pub fn crash() -> Self {
        Self {
            mode: DiagnosticMode::Crash,
            verification: DiagnosticVerificationPolicy::Quick,
            sources: DiagnosticSourcePolicy::default(),
            process_exit: None,
        }
    }

    /// Creates a full analysis request.
    #[must_use]
    pub fn full() -> Self {
        Self {
            mode: DiagnosticMode::Full,
            verification: DiagnosticVerificationPolicy::Full,
            sources: DiagnosticSourcePolicy::default(),
            process_exit: None,
        }
    }

    /// Attaches process exit evidence.
    #[must_use]
    pub fn with_process_exit(mut self, evidence: ProcessExitEvidence) -> Self {
        self.process_exit = Some(evidence);
        self
    }

    /// Validates the request and all nested limits.
    pub fn validate(&self) -> Result<()> {
        self.sources.validate()?;
        if let Some(exit) = &self.process_exit {
            exit.validate()?;
        }
        Ok(())
    }
}
