use crate::model::{Confidence, ContentEnvironment, EvidenceSourceKind, codes};
use crate::parser::{ParsedEvidence, ParsedFinding};
use graphene_content::{
    DependencyRelation, EnvironmentSupport, LocalContentInventory, LocalFileStatus,
    ModMetadataSource,
};
use graphene_core::{Diagnostic, DiagnosticCode, DiagnosticParameters, DiagnosticSeverity};
use std::collections::BTreeSet;

/// Mod ids provided by the platform/loader and never reported as missing local dependencies.
const BUILTIN_DEPENDENCIES: &[&str] = &[
    "minecraft",
    "java",
    "fabricloader",
    "fabric",
    "forge",
    "neoforge",
    "fml",
];

fn expected_loader(source: ModMetadataSource) -> Option<&'static str> {
    match source {
        ModMetadataSource::Fabric => Some("fabric"),
        ModMetadataSource::Forge => Some("forge"),
        ModMetadataSource::NeoForge => Some("neoforge"),
        ModMetadataSource::LegacyMcModInfo
        | ModMetadataSource::LegacyManifest
        | ModMetadataSource::Unknown => None,
    }
}

fn finding(
    code: &'static str,
    severity: DiagnosticSeverity,
    confidence: Confidence,
    parameters: DiagnosticParameters,
    evidence: Vec<ParsedEvidence>,
) -> ParsedFinding {
    ParsedFinding {
        diagnostic: Diagnostic {
            code: DiagnosticCode::new(code),
            severity,
            parameters,
        },
        confidence,
        evidence,
    }
}

fn structured_fields(
    source: EvidenceSourceKind,
    path: Option<graphene_instance::ManagedRelativePath>,
    pairs: &[(&str, &str)],
) -> ParsedEvidence {
    let mut fields = DiagnosticParameters::new();

    for (key, value) in pairs {
        fields.insert((*key).to_owned(), (*value).to_owned());
    }

    ParsedEvidence::structured(source, path, fields)
}

/// Analyzes a local mod inventory strictly offline for conflicts Graphene can justify.
///
/// Unknown or unsafe version ranges are preserved as raw evidence instead of being evaluated.
#[must_use]
pub fn correlate_content(
    inventory: &LocalContentInventory,
    environment: Option<&ContentEnvironment>,
) -> Vec<ParsedFinding> {
    let mut findings = Vec::new();

    let enabled_ids: BTreeSet<String> = inventory
        .files
        .iter()
        .filter(|file| file.enabled)
        .flat_map(|file| {
            file.descriptors
                .iter()
                .map(|descriptor| descriptor.mod_id.to_lowercase())
        })
        .collect();

    for duplicate in &inventory.duplicate_mod_ids {
        let mut parameters = DiagnosticParameters::new();
        parameters.insert("mod_id".to_owned(), duplicate.mod_id.clone());
        let mut evidence: Vec<ParsedEvidence> = duplicate
            .conflicting_files
            .iter()
            .map(|path| {
                structured_fields(
                    EvidenceSourceKind::ContentInventory,
                    Some(path.clone()),
                    &[("mod_id", duplicate.mod_id.as_str())],
                )
            })
            .collect();
        if evidence.is_empty() {
            evidence.push(structured_fields(
                EvidenceSourceKind::ContentInventory,
                None,
                &[("mod_id", duplicate.mod_id.as_str())],
            ));
        }
        findings.push(finding(
            codes::DUPLICATE_MOD_ID,
            DiagnosticSeverity::Error,
            Confidence::Confirmed,
            parameters,
            evidence,
        ));
    }

    for file in &inventory.files {
        if file.status == LocalFileStatus::Invalid {
            let mut parameters = DiagnosticParameters::new();
            parameters.insert("filename".to_owned(), file.filename.clone());
            let severity = DiagnosticSeverity::Warning;
            findings.push(finding(
                codes::MALFORMED_MOD_ARCHIVE,
                severity,
                Confidence::Strong,
                parameters,
                vec![structured_fields(
                    EvidenceSourceKind::ContentInventory,
                    Some(file.relative_path.clone()),
                    &[("filename", file.filename.as_str())],
                )],
            ));
        }

        if !file.enabled {
            continue;
        }

        for descriptor in &file.descriptors {
            if let Some(environment) = environment {
                if let Some(expected) = expected_loader(descriptor.source)
                    && let Some(actual) = environment.loader.as_deref().map(str::to_lowercase)
                    && actual != expected
                {
                    let mut parameters = DiagnosticParameters::new();
                    parameters.insert("mod_id".to_owned(), descriptor.mod_id.clone());
                    findings.push(finding(
                        codes::LOADER_ENVIRONMENT_MISMATCH,
                        DiagnosticSeverity::Error,
                        Confidence::Confirmed,
                        parameters,
                        vec![structured_fields(
                            EvidenceSourceKind::ContentInventory,
                            Some(file.relative_path.clone()),
                            &[
                                ("mod_id", descriptor.mod_id.as_str()),
                                ("declared_loader", expected),
                                ("instance_loader", actual.as_str()),
                            ],
                        )],
                    ));
                }

                if !environment.server && descriptor.environment == EnvironmentSupport::ServerOnly {
                    let mut parameters = DiagnosticParameters::new();
                    parameters.insert("mod_id".to_owned(), descriptor.mod_id.clone());
                    findings.push(finding(
                        codes::LOADER_ENVIRONMENT_MISMATCH,
                        DiagnosticSeverity::Error,
                        Confidence::Confirmed,
                        parameters,
                        vec![structured_fields(
                            EvidenceSourceKind::ContentInventory,
                            Some(file.relative_path.clone()),
                            &[
                                ("mod_id", descriptor.mod_id.as_str()),
                                ("declared_environment", "server_only"),
                            ],
                        )],
                    ));
                }
            }

            for dependency in &descriptor.dependencies {
                let normalized = dependency.mod_id.to_lowercase();
                match dependency.relation {
                    DependencyRelation::Required => {
                        if BUILTIN_DEPENDENCIES.contains(&normalized.as_str())
                            || enabled_ids.contains(&normalized)
                        {
                            continue;
                        }

                        let mut pairs = vec![
                            ("mod_id", descriptor.mod_id.as_str()),
                            ("dependency", dependency.mod_id.as_str()),
                        ];
                        if let Some(range) = &dependency.version_range {
                            pairs.push(("version_range", range.as_str()));
                        }

                        let mut parameters = DiagnosticParameters::new();
                        parameters.insert("mod_id".to_owned(), descriptor.mod_id.clone());
                        parameters.insert("dependency".to_owned(), dependency.mod_id.clone());
                        findings.push(finding(
                            codes::MISSING_REQUIRED_DEPENDENCY,
                            DiagnosticSeverity::Error,
                            Confidence::Strong,
                            parameters,
                            vec![structured_fields(
                                EvidenceSourceKind::ContentInventory,
                                Some(file.relative_path.clone()),
                                &pairs,
                            )],
                        ));
                    }

                    DependencyRelation::Incompatible => {
                        if !enabled_ids.contains(&normalized) {
                            continue;
                        }

                        let mut parameters = DiagnosticParameters::new();
                        parameters.insert("mod_id".to_owned(), descriptor.mod_id.clone());
                        parameters
                            .insert("incompatible_with".to_owned(), dependency.mod_id.clone());
                        findings.push(finding(
                            codes::CONTENT_INCOMPATIBILITY,
                            DiagnosticSeverity::Error,
                            Confidence::Confirmed,
                            parameters,
                            vec![structured_fields(
                                EvidenceSourceKind::ContentInventory,
                                Some(file.relative_path.clone()),
                                &[
                                    ("mod_id", descriptor.mod_id.as_str()),
                                    ("incompatible_with", dependency.mod_id.as_str()),
                                ],
                            )],
                        ));
                    }

                    DependencyRelation::Optional
                    | DependencyRelation::Embedded
                    | DependencyRelation::Unknown => {}
                }
            }
        }
    }

    findings
}
