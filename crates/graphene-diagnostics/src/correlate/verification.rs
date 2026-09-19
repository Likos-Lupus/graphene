use crate::model::{Confidence, EvidenceSourceKind, codes};
use crate::parser::{ParsedEvidence, ParsedFinding};
use graphene_core::{Diagnostic, DiagnosticCode, DiagnosticParameters, DiagnosticSeverity};
use graphene_instance::{FindingCode, VerificationReport};

fn map_code(code: FindingCode) -> (&'static str, DiagnosticSeverity, Confidence) {
    use FindingCode::{
        DescriptorInvalid, GeneratedOutputMismatch, GeneratedOutputMissing, IdentityMismatch,
        LegacyInstance, LockfileInvalid, LockfileMissing, ManagedFileHashMismatch,
        ManagedFileMissing, ManagedFileSizeMismatch, ManagedFileWrongType, NativeDirectoryMissing,
        ReceiptInvalid, RepairConflict, RepairSourceUnavailable, UnsafeSymlink,
    };

    match code {
        DescriptorInvalid => (
            codes::INSTANCE_DESCRIPTOR_INVALID,
            DiagnosticSeverity::Critical,
            Confidence::Confirmed,
        ),
        ReceiptInvalid => (
            codes::INSTALL_RECEIPT_INVALID,
            DiagnosticSeverity::Critical,
            Confidence::Confirmed,
        ),
        LockfileMissing => (
            codes::LOCKFILE_MISSING,
            DiagnosticSeverity::Warning,
            Confidence::Strong,
        ),
        LockfileInvalid => (
            codes::LOCKFILE_INVALID,
            DiagnosticSeverity::Critical,
            Confidence::Confirmed,
        ),
        IdentityMismatch => (
            codes::INSTANCE_IDENTITY_MISMATCH,
            DiagnosticSeverity::Error,
            Confidence::Confirmed,
        ),
        ManagedFileMissing => (
            codes::MANAGED_FILE_MISSING,
            DiagnosticSeverity::Error,
            Confidence::Confirmed,
        ),
        ManagedFileWrongType => (
            codes::MANAGED_FILE_WRONG_TYPE,
            DiagnosticSeverity::Critical,
            Confidence::Confirmed,
        ),
        ManagedFileSizeMismatch => (
            codes::MANAGED_FILE_SIZE_MISMATCH,
            DiagnosticSeverity::Error,
            Confidence::Confirmed,
        ),
        ManagedFileHashMismatch => (
            codes::MANAGED_FILE_HASH_MISMATCH,
            DiagnosticSeverity::Error,
            Confidence::Confirmed,
        ),
        GeneratedOutputMissing => (
            codes::GENERATED_OUTPUT_MISSING,
            DiagnosticSeverity::Error,
            Confidence::Confirmed,
        ),
        GeneratedOutputMismatch => (
            codes::GENERATED_OUTPUT_MISMATCH,
            DiagnosticSeverity::Error,
            Confidence::Confirmed,
        ),
        NativeDirectoryMissing => (
            codes::NATIVE_DIRECTORY_MISSING,
            DiagnosticSeverity::Error,
            Confidence::Confirmed,
        ),
        UnsafeSymlink => (
            codes::UNSAFE_SYMLINK,
            DiagnosticSeverity::Critical,
            Confidence::Confirmed,
        ),
        LegacyInstance => (
            codes::LEGACY_DESIRED_STATE,
            DiagnosticSeverity::Warning,
            Confidence::Confirmed,
        ),
        RepairSourceUnavailable => (
            codes::REPAIR_SOURCE_UNAVAILABLE,
            DiagnosticSeverity::Error,
            Confidence::Strong,
        ),
        RepairConflict => (
            codes::REPAIR_CONFLICT,
            DiagnosticSeverity::Error,
            Confidence::Strong,
        ),
    }
}

/// Maps a verification report into normalized findings without replanning or executing repair.
#[must_use]
pub fn correlate_verification(report: &VerificationReport) -> Vec<ParsedFinding> {
    report
        .findings
        .iter()
        .map(|finding| {
            let (code, severity, confidence) = map_code(finding.code);
            let mut fields = DiagnosticParameters::new();
            fields.insert(
                "verification_code".to_owned(),
                format!("{:?}", finding.code),
            );

            fields.insert("repairable".to_owned(), finding.repairable.to_string());
            if let Some(logical_item) = &finding.logical_item {
                fields.insert("logical_item".to_owned(), logical_item.clone());
            }

            ParsedFinding {
                diagnostic: Diagnostic {
                    code: DiagnosticCode::new(code),
                    severity,
                    parameters: DiagnosticParameters::new(),
                },
                confidence,
                evidence: vec![ParsedEvidence::structured(
                    EvidenceSourceKind::VerificationReport,
                    finding.path.clone(),
                    fields,
                )],
            }
        })
        .collect()
}
