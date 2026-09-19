use super::{ParsedEvidence, ParsedFinding};
use crate::analysis::redaction::Redactor;
use crate::analysis::text::bounded_lines;
use crate::model::{Confidence, EvidenceSourceKind, codes};
use graphene_core::{Diagnostic, DiagnosticCode, DiagnosticParameters, DiagnosticSeverity};

pub(crate) struct Rule {
    pub code: &'static str,
    pub severity: DiagnosticSeverity,
    pub confidence: Confidence,
    pub markers: &'static [&'static str],
    pub source_trigger: Option<EvidenceSourceKind>,
    pub extract: fn(&str) -> DiagnosticParameters,
}

fn empty_fields(_: &str) -> DiagnosticParameters {
    DiagnosticParameters::new()
}

fn extract_class_major(text: &str) -> DiagnosticParameters {
    let mut parameters = DiagnosticParameters::new();
    let lower = text.to_ascii_lowercase();

    if let Some(position) = lower.find("class file version") {
        let rest = &text[position + "class file version".len()..];
        let digits: String = rest
            .chars()
            .skip_while(|character| !character.is_ascii_digit())
            .take_while(char::is_ascii_digit)
            .collect();

        if !digits.is_empty() {
            parameters.insert("class_major".to_owned(), digits);
        }
    }

    parameters
}

/// Ordered, defensive rule families. Markers are lowercase substrings; evidence is always
/// excerpted and redacted before exposure.
pub(crate) const RULES: &[Rule] = &[
    Rule {
        code: codes::JVM_OUT_OF_MEMORY,
        severity: DiagnosticSeverity::Error,
        confidence: Confidence::Strong,
        markers: &[
            "outofmemoryerror",
            "java heap space",
            "gc overhead limit exceeded",
            "unable to create native thread",
            "direct buffer memory",
            "native memory allocation (mmap) failed",
            "insufficient memory for the java runtime",
            "there is insufficient memory",
        ],
        source_trigger: None,
        extract: empty_fields,
    },
    Rule {
        code: codes::JAVA_CLASS_VERSION_MISMATCH,
        severity: DiagnosticSeverity::Error,
        confidence: Confidence::Strong,
        markers: &[
            "unsupportedclassversionerror",
            "has been compiled by a more recent version of the java runtime",
            "class file version",
        ],
        source_trigger: None,
        extract: extract_class_major,
    },
    Rule {
        code: codes::CLASS_LOADING_FAILURE,
        severity: DiagnosticSeverity::Error,
        confidence: Confidence::Strong,
        markers: &["classnotfoundexception", "noclassdeffounderror"],
        source_trigger: None,
        extract: empty_fields,
    },
    Rule {
        code: codes::LOADER_MISSING_DEPENDENCY,
        severity: DiagnosticSeverity::Error,
        confidence: Confidence::Strong,
        markers: &[
            "missing or unsupported mandatory dependencies",
            "requires any version of",
            "dependency satisfaction failed",
            "incompatible mod set",
            "mod file is missing",
            "missing mods",
            "missing dependency",
            "unsatisfied dependency",
        ],
        source_trigger: None,
        extract: empty_fields,
    },
    Rule {
        code: codes::DUPLICATE_MOD_ID,
        severity: DiagnosticSeverity::Error,
        confidence: Confidence::Strong,
        markers: &[
            "duplicate mod",
            "duplicate mod id",
            "is already loaded",
            "duplicate id",
            "duplicate entry",
        ],
        source_trigger: None,
        extract: empty_fields,
    },
    Rule {
        code: codes::LOADER_LOAD_FAILURE,
        severity: DiagnosticSeverity::Error,
        confidence: Confidence::Strong,
        markers: &[
            "failed to load mod",
            "could not load mod",
            "error loading mod",
            "mod loading has failed",
            "mod loading failure",
            "exception loading mod",
            "an error occurred while loading",
        ],
        source_trigger: None,
        extract: empty_fields,
    },
    Rule {
        code: codes::MIXIN_FAILURE,
        severity: DiagnosticSeverity::Error,
        confidence: Confidence::Heuristic,
        markers: &[
            "mixin apply failed",
            "mixinapplyerror",
            "invalidmixinexception",
            "org.spongepowered.asm.mixin",
            "mixin transformation",
            "mixin failed",
            "mixin error",
        ],
        source_trigger: None,
        extract: empty_fields,
    },
    Rule {
        code: codes::NATIVE_LIBRARY_FAILURE,
        severity: DiagnosticSeverity::Error,
        confidence: Confidence::Strong,
        markers: &[
            "unsatisfiedlinkerror",
            "failed to load natives",
            "no lwjgl in java.library.path",
            "could not load library",
            "error loading native",
            "nativelibrary",
            "loadlibrary failed",
        ],
        source_trigger: None,
        extract: empty_fields,
    },
    Rule {
        code: codes::GRAPHICS_INITIALIZATION_FAILURE,
        severity: DiagnosticSeverity::Error,
        confidence: Confidence::Strong,
        markers: &[
            "failed to create glfw window",
            "glfw error",
            "could not create gl context",
            "failed to initialize opengl",
            "org.lwjgl.opengl",
        ],
        source_trigger: None,
        extract: empty_fields,
    },
    Rule {
        code: codes::JVM_FATAL_ERROR,
        severity: DiagnosticSeverity::Critical,
        confidence: Confidence::Strong,
        markers: &[
            "a fatal error has been detected by the java runtime environment",
            "exception_access_violation",
            "# a fatal error",
        ],
        source_trigger: Some(EvidenceSourceKind::HsErrLog),
        extract: empty_fields,
    },
];

pub(crate) fn detect_rule(
    rule: &Rule,
    sources: &[super::TextSource],
    redactor: &Redactor,
    max_excerpt_bytes: usize,
    max_line_bytes: usize,
    max_evidence: usize,
) -> Option<ParsedFinding> {
    if let Some(kind) = rule.source_trigger {
        let source = sources.iter().find(|source| source.source == kind)?;
        let excerpt = redactor.excerpt(&source.text, max_excerpt_bytes);

        return Some(ParsedFinding {
            diagnostic: Diagnostic {
                code: DiagnosticCode::new(rule.code),
                severity: rule.severity,
                parameters: (rule.extract)(&source.text),
            },
            confidence: rule.confidence,
            evidence: vec![ParsedEvidence {
                source: kind,
                path: source.path.clone(),
                line: None,
                excerpt: Some(excerpt.text),
                truncated: excerpt.truncated || source.truncated,
                fields: DiagnosticParameters::new(),
            }],
        });
    }

    let mut evidence = Vec::new();
    'outer: for source in sources {
        for line in bounded_lines(&source.text, max_line_bytes) {
            let lower = line.text.to_ascii_lowercase();
            if rule.markers.iter().any(|marker| lower.contains(marker)) {
                let excerpt = redactor.excerpt(&line.text, max_excerpt_bytes);
                evidence.push(ParsedEvidence {
                    source: source.source,
                    path: source.path.clone(),
                    line: Some(line.number),
                    excerpt: Some(excerpt.text),
                    truncated: excerpt.truncated || line.truncated,
                    fields: DiagnosticParameters::new(),
                });
                if evidence.len() >= max_evidence {
                    break 'outer;
                }
            }
        }
    }

    if evidence.is_empty() {
        return None;
    }

    let parameters = evidence
        .first()
        .map(|evidence| (rule.extract)(evidence.excerpt.as_deref().unwrap_or_default()))
        .unwrap_or_default();

    Some(ParsedFinding {
        diagnostic: Diagnostic {
            code: DiagnosticCode::new(rule.code),
            severity: rule.severity,
            parameters,
        },
        confidence: rule.confidence,
        evidence,
    })
}
