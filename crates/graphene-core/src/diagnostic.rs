use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable diagnostic code owned by Graphene rather than a UI localization layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiagnosticCode(String);

impl DiagnosticCode {
    /// Creates a diagnostic code. Callers should use stable uppercase identifiers.
    #[must_use]
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    /// Returns the code string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Diagnostic severity independent of presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Structured parameters that a host may localize and present.
pub type DiagnosticParameters = BTreeMap<String, String>;

/// UI-independent structured diagnostic evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: DiagnosticSeverity,
    pub parameters: DiagnosticParameters,
}
