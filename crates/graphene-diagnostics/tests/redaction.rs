use graphene_core::SensitiveString;
use graphene_diagnostics::{
    DATA_ROOT_PLACEHOLDER, DiagnosticEvidence, EvidenceId, EvidenceSourceKind, HOME_PLACEHOLDER,
    RedactionContext, Redactor, SECRET_PLACEHOLDER, bounded_lines, decode_lossy,
};

const SENTINEL: &str = "GRAPHENE-SENTINEL-SECRET-9f3a";

fn redactor() -> Redactor {
    let context = RedactionContext::new()
        .with_secret(SensitiveString::new(SENTINEL))
        .with_secret(SensitiveString::new("supersecretvalue"))
        .with_secret(SensitiveString::new("secretvalue"))
        .with_data_root("/home/user/.graphene")
        .with_home("/home/user");
    Redactor::new(&context)
}

#[test]
fn known_secret_is_removed() {
    let redactor = redactor();
    let output = redactor.redact(&format!("start {SENTINEL} end"));

    assert!(!output.contains(SENTINEL));
    assert!(output.contains(SECRET_PLACEHOLDER));
}

#[test]
fn overlapping_substring_secrets_use_longest_match_first() {
    let redactor = redactor();
    let output = redactor.redact("value=supersecretvalue");

    assert!(!output.contains("supersecretvalue"));
    assert!(!output.contains("secretvalue"));
    assert_eq!(output, format!("value={SECRET_PLACEHOLDER}"));
}

#[test]
fn bearer_and_authorization_forms_are_redacted() {
    let redactor = redactor();
    let output = redactor.redact(
        "Authorization: Bearer abc.def.ghi\nstandalone Bearer xyz-token-123\nBasic dXNlcjpwYXNz",
    );

    assert!(!output.contains("abc.def.ghi"));
    assert!(!output.contains("xyz-token-123"));
    assert!(!output.contains("dXNlcjpwYXNz"));
}

#[test]
fn key_value_secret_shapes_are_redacted() {
    let redactor = redactor();
    let cases = [
        "access_token=AAA111",
        "\"refresh_token\": \"BBB222\"",
        "client_secret: CCC333",
        "api_key=DDD444",
        "x-api-key=EEE555",
        "password=hunter2",
        "device_code=FFF666",
    ];

    for case in cases {
        let output = redactor.redact(case);
        assert!(output.contains(SECRET_PLACEHOLDER), "case: {case}");
        for value in [
            "AAA111", "BBB222", "CCC333", "DDD444", "EEE555", "hunter2", "FFF666",
        ] {
            assert!(!output.contains(value), "case: {case}");
        }
    }
}

#[test]
fn url_userinfo_and_query_credentials_are_redacted() {
    let redactor = redactor();
    let output =
        redactor.redact("fetch https://alice:s3cr3t@example.com/path?access_token=tok123&x=1 now");

    assert!(!output.contains("s3cr3t"));
    assert!(!output.contains("alice"));
    assert!(!output.contains("tok123"));
    assert!(output.contains("example.com"));
}

#[test]
fn data_root_and_home_prefixes_become_stable_placeholders() {
    let redactor = redactor();
    let output = redactor.redact("/home/user/.graphene/instances/abc/logs/latest.log");

    assert_eq!(
        output,
        format!("{DATA_ROOT_PLACEHOLDER}/instances/abc/logs/latest.log")
    );

    let home_only = redactor.redact("/home/user/Documents/notes.txt");

    assert_eq!(home_only, format!("{HOME_PLACEHOLDER}/Documents/notes.txt"));
}

#[test]
fn redaction_is_idempotent() {
    let redactor = redactor();
    let input = format!(
        "Authorization: Bearer abc.def\naccess_token=AAA111\nhttps://alice:s3cr3t@example.com/?token=tok\n{SENTINEL}\n/home/user/.graphene/x"
    );
    let once = redactor.redact(&input);
    let twice = redactor.redact(&once);

    assert_eq!(once, twice);
}

#[test]
fn debug_output_never_leaks_sentinels() {
    let context = RedactionContext::new().with_secret(SensitiveString::new(SENTINEL));

    assert!(!format!("{context:?}").contains(SENTINEL));

    let redactor = Redactor::new(&context);

    assert!(!format!("{redactor:?}").contains(SENTINEL));
}

#[test]
fn serialized_evidence_contains_only_redacted_excerpts() {
    let redactor = redactor();
    let excerpt = redactor.excerpt(&format!("token x {SENTINEL} tail"), 4096);
    let evidence = DiagnosticEvidence::structured(
        EvidenceId::new(1),
        EvidenceSourceKind::LogFile,
        Default::default(),
    )
    .with_excerpt(excerpt.text, excerpt.truncated);
    let json = serde_json::to_string(&evidence).expect("serialize");

    assert!(!json.contains(SENTINEL));
}

#[test]
fn excerpt_bounds_to_char_boundary() {
    let redactor = redactor();
    let excerpt = redactor.excerpt("éééééééééé", 5);

    assert!(excerpt.truncated);
    assert!(excerpt.text.len() <= 5);
    assert!(excerpt.text.is_char_boundary(excerpt.text.len()));
}

#[test]
fn lossy_decode_and_line_scanning_tolerate_hostile_input() {
    let bytes = b"line one\r\n\xFF\xFE invalid\nno trailing newline";
    let text = decode_lossy(bytes);
    let lines: Vec<_> = bounded_lines(&text, 4).collect();

    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].text, "line");
    assert!(lines[0].truncated);
    assert_eq!(lines[2].text, "no t");
    assert!(lines[2].truncated);
}
