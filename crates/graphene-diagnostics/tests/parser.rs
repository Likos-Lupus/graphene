use graphene_core::{DiagnosticSeverity, SensitiveString};
use graphene_diagnostics::{
    Confidence, EvidenceSourceKind, ParseLimits, ProcessExitEvidence, RedactionContext, Redactor,
    TextSource, cause_undetermined_finding, decode_lossy, has_severe_finding, parse_sources,
    process_exit_finding,
};

fn parse(fixture: &str, source: EvidenceSourceKind) -> Vec<graphene_diagnostics::ParsedFinding> {
    parse_with(Redactor::new(&RedactionContext::new()), fixture, source)
}

fn parse_with(
    redactor: Redactor,
    fixture: &str,
    source: EvidenceSourceKind,
) -> Vec<graphene_diagnostics::ParsedFinding> {
    let sources = [TextSource::new(source, None, fixture)];
    parse_sources(&sources, &redactor, &ParseLimits::default())
}

fn codes(findings: &[graphene_diagnostics::ParsedFinding]) -> Vec<String> {
    findings
        .iter()
        .map(|finding| finding.diagnostic.code.as_str().to_owned())
        .collect()
}

fn find<'a>(
    findings: &'a [graphene_diagnostics::ParsedFinding],
    code: &str,
) -> &'a graphene_diagnostics::ParsedFinding {
    findings
        .iter()
        .find(|finding| finding.diagnostic.code.as_str() == code)
        .unwrap_or_else(|| panic!("missing finding {code}"))
}

#[test]
fn out_of_memory_is_strong_and_error_severity() {
    let findings = parse(
        include_str!("fixtures/out_of_memory.log"),
        EvidenceSourceKind::LogFile,
    );
    let finding = find(&findings, "DIAGNOSTIC_JVM_OUT_OF_MEMORY");

    assert_eq!(finding.confidence, Confidence::Strong);
    assert_eq!(finding.diagnostic.severity, DiagnosticSeverity::Error);
    assert!(!finding.evidence.is_empty());
    assert_eq!(finding.evidence[0].line, Some(0));
}

#[test]
fn unsupported_class_version_extracts_major() {
    let findings = parse(
        include_str!("fixtures/unsupported_class_version.log"),
        EvidenceSourceKind::LogFile,
    );
    let finding = find(&findings, "DIAGNOSTIC_JAVA_CLASS_VERSION_MISMATCH");

    assert_eq!(
        finding
            .diagnostic
            .parameters
            .get("class_major")
            .map(String::as_str),
        Some("61")
    );
}

#[test]
fn fabric_missing_dependency_is_detected() {
    let findings = parse(
        include_str!("fixtures/fabric_missing_dependency.log"),
        EvidenceSourceKind::LogFile,
    );

    assert!(codes(&findings).contains(&"DIAGNOSTIC_LOADER_MISSING_DEPENDENCY".to_owned()));
}

#[test]
fn forge_loading_failure_emits_independent_findings() {
    let findings = parse(
        include_str!("fixtures/forge_mod_loading_failure.log"),
        EvidenceSourceKind::LogFile,
    );
    let codes = codes(&findings);

    assert!(codes.contains(&"DIAGNOSTIC_LOADER_LOAD_FAILURE".to_owned()));
    assert!(codes.contains(&"DIAGNOSTIC_CLASS_LOADING_FAILURE".to_owned()));
}

#[test]
fn mixin_failure_is_heuristic_not_confirmed() {
    let findings = parse(
        include_str!("fixtures/mixin_failure.log"),
        EvidenceSourceKind::LogFile,
    );
    let finding = find(&findings, "DIAGNOSTIC_MIXIN_FAILURE");

    assert_eq!(finding.confidence, Confidence::Heuristic);
}

#[test]
fn native_and_graphics_failures_are_both_detected() {
    let findings = parse(
        include_str!("fixtures/native_glfw_failure.log"),
        EvidenceSourceKind::LogFile,
    );
    let codes = codes(&findings);

    assert!(codes.contains(&"DIAGNOSTIC_NATIVE_LIBRARY_FAILURE".to_owned()));
    assert!(codes.contains(&"DIAGNOSTIC_GRAPHICS_INITIALIZATION_FAILURE".to_owned()));
}

#[test]
fn hs_err_presence_emits_critical_fatal_finding() {
    let findings = parse(
        include_str!("fixtures/hs_err_pid1234.log"),
        EvidenceSourceKind::HsErrLog,
    );
    let finding = find(&findings, "DIAGNOSTIC_JVM_FATAL_ERROR");

    assert_eq!(finding.diagnostic.severity, DiagnosticSeverity::Critical);
}

#[test]
fn unknown_vanilla_crash_does_not_fabricate_a_cause() {
    let findings = parse(
        include_str!("fixtures/vanilla_crash.txt"),
        EvidenceSourceKind::CrashReport,
    );

    assert!(!has_severe_finding(&findings));
    assert!(
        !codes(&findings)
            .iter()
            .any(|code| code.contains("JVM") || code.contains("JAVA"))
    );

    let undetermined = cause_undetermined_finding();

    assert_eq!(
        undetermined.diagnostic.code.as_str(),
        "DIAGNOSTIC_CAUSE_UNDETERMINED"
    );
    assert_eq!(undetermined.confidence, Confidence::Heuristic);
}

#[test]
fn truncated_and_invalid_utf8_input_is_panic_free_and_bounded() {
    let bytes = b"java.lang.OutOfMemoryError: Java heap space\xFF\xFE\x00";
    let text = decode_lossy(bytes);
    let limits = ParseLimits {
        max_line_bytes: 32,
        max_excerpt_bytes: 16,
        ..ParseLimits::default()
    };
    let findings = parse_sources(
        &[TextSource::new(EvidenceSourceKind::LogFile, None, text)],
        &Redactor::new(&RedactionContext::new()),
        &limits,
    );

    assert!(!findings.is_empty());

    for evidence in findings.iter().flat_map(|finding| &finding.evidence) {
        assert!(evidence.excerpt.as_deref().map_or(0, str::len) <= 16);
    }
}

#[test]
fn evidence_excerpts_are_redacted() {
    let sentinel = "SENTINEL-OOM-SECRET-123";
    let context = RedactionContext::new().with_secret(SensitiveString::new(sentinel));
    let fixture = format!("access_token={sentinel} java.lang.OutOfMemoryError: Java heap space");
    let findings = parse_with(
        Redactor::new(&context),
        &fixture,
        EvidenceSourceKind::LogFile,
    );
    let json = serde_json::to_string(&findings).expect("serialize findings");

    assert!(!json.contains(sentinel));
}

#[test]
fn non_zero_process_exit_produces_heuristic_finding() {
    let exit = ProcessExitEvidence::new(Some(1), false, false);
    let finding = process_exit_finding(&exit).expect("non-zero exit finding");

    assert_eq!(
        finding.diagnostic.code.as_str(),
        "DIAGNOSTIC_PROCESS_EXIT_NONZERO"
    );
    assert_eq!(finding.confidence, Confidence::Heuristic);
    assert_eq!(
        finding
            .diagnostic
            .parameters
            .get("exit_code")
            .map(String::as_str),
        Some("1")
    );

    assert!(process_exit_finding(&ProcessExitEvidence::new(Some(0), true, false)).is_none());
    assert!(process_exit_finding(&ProcessExitEvidence::new(None, false, true)).is_none());
}
