use super::{LoaderKind, LoaderSupport, LoaderVersion};
use crate::MinecraftVersionId;
use graphene_core::{Diagnostic, DiagnosticCode, DiagnosticParameters, DiagnosticSeverity};

/// Produces a stable, UI-independent loader support diagnostic when support is limited.
pub fn loader_support_diagnostic(
    kind: LoaderKind,
    version: &LoaderVersion,
    minecraft: &MinecraftVersionId,
    support: &LoaderSupport,
) -> Option<Diagnostic> {
    let (code, severity, reason) = match support {
        LoaderSupport::Supported => return None,
        LoaderSupport::MetadataOnly { reason } => {
            ("LOADER_METADATA_ONLY", DiagnosticSeverity::Warning, reason)
        }
        LoaderSupport::Unsupported { reason } => (
            "LOADER_VERSION_UNSUPPORTED",
            DiagnosticSeverity::Error,
            reason,
        ),
    };

    let mut parameters = DiagnosticParameters::new();
    parameters.insert("loader".to_owned(), kind.to_string());
    parameters.insert("loader_version".to_owned(), version.to_string());
    parameters.insert("minecraft_version".to_owned(), minecraft.to_string());
    parameters.insert("reason".to_owned(), reason.clone());

    Some(Diagnostic {
        code: DiagnosticCode::new(code),
        severity,
        parameters,
    })
}
