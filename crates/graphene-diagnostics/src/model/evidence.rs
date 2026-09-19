use graphene_core::DiagnosticParameters;
use graphene_instance::ManagedRelativePath;
use serde::{Deserialize, Serialize};

/// Stable report-local identifier for one diagnostic evidence record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EvidenceId(u32);

impl EvidenceId {
    /// Creates an evidence identifier.
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

/// Broad origin class of one diagnostic evidence record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceSourceKind {
    InstanceMetadata,
    InstallReceipt,
    DesiredStateLockfile,
    InstanceConfig,
    VerificationReport,
    JavaEvidence,
    LogFile,
    CrashReport,
    HsErrLog,
    ContentInventory,
    ProcessExit,
}

/// One bounded, redacted evidence record referenced by findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticEvidence {
    pub id: EvidenceId,
    pub source: EvidenceSourceKind,
    pub path: Option<ManagedRelativePath>,
    pub line: Option<u64>,
    pub excerpt: Option<String>,
    pub truncated: bool,
    pub fields: DiagnosticParameters,
}

impl DiagnosticEvidence {
    /// Creates a structured evidence record without a text excerpt.
    #[must_use]
    pub fn structured(
        id: EvidenceId,
        source: EvidenceSourceKind,
        fields: DiagnosticParameters,
    ) -> Self {
        Self {
            id,
            source,
            path: None,
            line: None,
            excerpt: None,
            truncated: false,
            fields,
        }
    }

    /// Attaches a redacted bounded excerpt.
    #[must_use]
    pub fn with_excerpt(mut self, excerpt: impl Into<String>, truncated: bool) -> Self {
        self.excerpt = Some(excerpt.into());
        self.truncated = truncated;
        self
    }

    /// Attaches a managed-relative source path.
    #[must_use]
    pub fn with_path(mut self, path: ManagedRelativePath) -> Self {
        self.path = Some(path);
        self
    }

    /// Attaches a source line or range start.
    #[must_use]
    pub fn with_line(mut self, line: u64) -> Self {
        self.line = Some(line);
        self
    }
}
