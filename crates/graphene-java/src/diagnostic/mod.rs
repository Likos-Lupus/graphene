use crate::{JavaArchitecture, JavaRequirement, JavaRuntime, JavaVendor};
use graphene_core::{Diagnostic, DiagnosticCode, DiagnosticParameters, DiagnosticSeverity};

/// Produces stable, UI-independent compatibility evidence for one requirement/runtime pair.
#[must_use]
pub fn compatibility_diagnostic(
    requirement: &JavaRequirement,
    runtime: Option<&JavaRuntime>,
) -> Diagnostic {
    let mut parameters = DiagnosticParameters::new();
    parameters.insert(
        "required_major".into(),
        requirement.major_version.to_string(),
    );
    parameters.insert(
        "required_architecture".into(),
        architecture_label(JavaArchitecture::current()).into(),
    );
    let (code, severity) = match runtime {
        None => ("JAVA_NOT_FOUND", DiagnosticSeverity::Warning),
        Some(runtime) if runtime.major_version < requirement.major_version => {
            parameters.insert("found_major".into(), runtime.major_version.to_string());
            ("JAVA_VERSION_TOO_OLD", DiagnosticSeverity::Warning)
        }
        Some(runtime) if runtime.major_version > requirement.major_version => {
            parameters.insert("found_major".into(), runtime.major_version.to_string());
            ("JAVA_VERSION_TOO_NEW", DiagnosticSeverity::Warning)
        }
        Some(runtime) if runtime.architecture != JavaArchitecture::current() => {
            parameters.insert(
                "found_architecture".into(),
                architecture_label(runtime.architecture).into(),
            );
            ("JAVA_ARCHITECTURE_MISMATCH", DiagnosticSeverity::Warning)
        }
        Some(runtime) => {
            parameters.insert("found_major".into(), runtime.major_version.to_string());
            parameters.insert("vendor".into(), vendor_label(&runtime.vendor).into());
            ("JAVA_COMPATIBLE", DiagnosticSeverity::Info)
        }
    };
    Diagnostic {
        code: DiagnosticCode::new(code),
        severity,
        parameters,
    }
}

const fn architecture_label(architecture: JavaArchitecture) -> &'static str {
    match architecture {
        JavaArchitecture::X86 => "x86",
        JavaArchitecture::X86_64 => "x86_64",
        JavaArchitecture::AArch64 => "aarch64",
        JavaArchitecture::Other => "other",
    }
}

fn vendor_label(vendor: &JavaVendor) -> &'static str {
    match vendor {
        JavaVendor::Adoptium => "adoptium",
        JavaVendor::Oracle => "oracle",
        JavaVendor::Microsoft => "microsoft",
        JavaVendor::Azul => "azul",
        JavaVendor::Amazon => "amazon",
        JavaVendor::GraalVm => "graalvm",
        JavaVendor::Other(_) => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn diagnostics_do_not_copy_arbitrary_vendor_text() {
        let requirement = JavaRequirement {
            major_version: 21,
            component_hint: None,
        };
        let runtime = JavaRuntime {
            executable: PathBuf::from("java"),
            version: "21".into(),
            major_version: 21,
            vendor: JavaVendor::Other("PROVIDER_PRIVATE_TEXT_DO_NOT_EXPORT".into()),
            architecture: JavaArchitecture::current(),
            java_home: None,
        };
        let diagnostic = compatibility_diagnostic(&requirement, Some(&runtime));
        assert_eq!(
            diagnostic.parameters.get("vendor").map(String::as_str),
            Some("other")
        );
        assert!(!format!("{diagnostic:?}").contains("PROVIDER_PRIVATE_TEXT_DO_NOT_EXPORT"));
    }
}
