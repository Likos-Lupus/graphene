use crate::{InstanceStateFingerprint, ManagedRelativePath};
use graphene_core::InstanceId;
use serde::{Deserialize, Serialize};

pub const MAX_VERIFICATION_FINDINGS: usize = 1000;

/// Verification inspection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerificationMode {
    /// Quick structural, semantic, and size checks without hashing large archives.
    #[default]
    Quick,
    /// Complete cryptographic integrity verification of all Graphene-managed artifacts.
    Full,
}

/// Overall repairability assessment for an instance based on findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Repairability {
    /// Fully healthy instance: no repair needed.
    Healthy,
    /// Damaged but can be reconstructed entirely using local cache / artifacts.
    LocallyRepairable,
    /// Damaged and requires downloading missing artifacts from remote sources.
    RepairableWithNetwork,
    /// Damaged and requires executing loader processor tools (tool Java required).
    RepairableWithPreparation,
    /// Pre-Phase-4 legacy instance: requires explicit desired-state lockfile creation/migration.
    LegacyMigrationRequired,
    /// Irreparable corruption: missing required metadata or unrepairable conflict.
    Unrepairable,
}

/// Severity classification for a verification finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FindingSeverity {
    Info,
    Warning,
    Error,
}

/// Stable machine-readable code for a verification finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FindingCode {
    DescriptorInvalid,
    ReceiptInvalid,
    LockfileMissing,
    LockfileInvalid,
    IdentityMismatch,
    ManagedFileMissing,
    ManagedFileWrongType,
    ManagedFileSizeMismatch,
    ManagedFileHashMismatch,
    GeneratedOutputMissing,
    GeneratedOutputMismatch,
    NativeDirectoryMissing,
    UnsafeSymlink,
    LegacyInstance,
    RepairSourceUnavailable,
    RepairConflict,
}

/// An individual diagnostic finding discovered during instance verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationFinding {
    pub code: FindingCode,
    pub severity: FindingSeverity,
    pub path: Option<ManagedRelativePath>,
    pub logical_item: Option<String>,
    pub message: String,
    pub repairable: bool,
}

impl VerificationFinding {
    /// Creates a new verification finding.
    #[must_use]
    pub fn new(
        code: FindingCode,
        severity: FindingSeverity,
        message: impl Into<String>,
        repairable: bool,
    ) -> Self {
        Self {
            code,
            severity,
            path: None,
            logical_item: None,
            message: message.into(),
            repairable,
        }
    }

    /// Attaches an affected managed-relative path.
    #[must_use]
    pub fn with_path(mut self, path: ManagedRelativePath) -> Self {
        self.path = Some(path);
        self
    }

    /// Attaches an affected logical item identifier.
    #[must_use]
    pub fn with_logical_item(mut self, item: impl Into<String>) -> Self {
        self.logical_item = Some(item.into());
        self
    }
}

/// Structured report returned by the verification engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationReport {
    pub instance_id: InstanceId,
    pub mode: VerificationMode,
    pub state_fingerprint: InstanceStateFingerprint,
    pub repairability: Repairability,
    pub findings: Vec<VerificationFinding>,
    pub summary: String,
}

impl VerificationReport {
    /// Returns whether the instance is completely healthy (no error-severity findings).
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        !self
            .findings
            .iter()
            .any(|f| f.severity == FindingSeverity::Error)
    }

    /// Returns whether this report indicates the instance can be repaired.
    #[must_use]
    pub fn is_repairable(&self) -> bool {
        matches!(
            self.repairability,
            Repairability::Healthy
                | Repairability::LocallyRepairable
                | Repairability::RepairableWithNetwork
                | Repairability::RepairableWithPreparation
        )
    }
}
