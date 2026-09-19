use super::collection;
use graphene_core::{CancellationToken, ErrorCode, InstanceId};
use graphene_diagnostics::{
    DiagnosticCompleteness, DiagnosticSourcePolicy, EvidenceSourceKind,
};
use graphene_storage::InstancePaths;
use std::fs;

fn instance_id() -> InstanceId {
    InstanceId::from_bytes([9; 16])
}

fn paths(temp: &tempfile::TempDir) -> InstancePaths {
    InstancePaths::new(temp.path())
}

#[test]
fn collects_allowlisted_sources_and_truncates_oversized_files() {
    let temp = tempfile::tempdir().unwrap();
    let paths = paths(&temp);
    let id = instance_id();
    let minecraft = paths.minecraft_dir(id);

    fs::create_dir_all(minecraft.join("logs")).unwrap();
    fs::create_dir_all(minecraft.join("crash-reports")).unwrap();
    fs::write(minecraft.join("logs/latest.log"), "hello\n").unwrap();
    fs::write(minecraft.join("crash-reports/crash-a.txt"), "crash a\n").unwrap();
    fs::write(minecraft.join("crash-reports/crash-b.txt"), "crash b\n").unwrap();
    fs::write(minecraft.join("hs_err_pid1.log"), "fatal\n").unwrap();
    fs::write(minecraft.join("logs/debug.log"), "x".repeat(2048)).unwrap();

    let policy = DiagnosticSourcePolicy {
        max_source_bytes: 64,
        max_total_bytes: 4096,
        ..DiagnosticSourcePolicy::default()
    };
    let collected =
        collection::collect_sources(&paths, id, &policy, &CancellationToken::new()).unwrap();

    assert_eq!(collected.text_sources.len(), 5);
    assert_eq!(collected.completeness, DiagnosticCompleteness::Partial);
    let debug = collected
        .text_sources
        .iter()
        .find(|source| {
            source
                .path
                .as_ref()
                .map(graphene_instance::ManagedRelativePath::as_str)
                == Some(".minecraft/logs/debug.log")
        })
        .expect("debug log source");
    assert!(debug.truncated);
    assert!(debug.text.len() <= 64);
}

#[test]
fn crash_report_selection_is_bounded_and_deterministic() {
    let temp = tempfile::tempdir().unwrap();
    let paths = paths(&temp);
    let id = instance_id();
    let crash_dir = paths.minecraft_dir(id).join("crash-reports");

    fs::create_dir_all(&crash_dir).unwrap();

    for index in 0..10 {
        fs::write(crash_dir.join(format!("crash-{index:02}.txt")), "crash\n").unwrap();
    }

    let policy = DiagnosticSourcePolicy::default();
    let collected =
        collection::collect_sources(&paths, id, &policy, &CancellationToken::new()).unwrap();
    let crash_sources: Vec<_> = collected
        .text_sources
        .iter()
        .filter(|source| source.source == EvidenceSourceKind::CrashReport)
        .collect();
    assert_eq!(crash_sources.len(), policy.max_crash_reports);

    let first_content = &crash_sources[0].text;
    let second = collection::collect_sources(&paths, id, &policy, &CancellationToken::new())
        .unwrap()
        .text_sources;
    assert_eq!(
        second
            .iter()
            .find(|source| source.source == EvidenceSourceKind::CrashReport)
            .map(|source| &source.text),
        Some(first_content)
    );
}

#[test]
fn missing_sources_are_skipped_without_error() {
    let temp = tempfile::tempdir().unwrap();
    let paths = paths(&temp);
    let id = instance_id();
    fs::create_dir_all(paths.instance_root(id)).unwrap();

    let collected = collection::collect_sources(
        &paths,
        id,
        &DiagnosticSourcePolicy::default(),
        &CancellationToken::new(),
    )
    .unwrap();
    assert!(collected.text_sources.is_empty());
    assert_eq!(collected.completeness, DiagnosticCompleteness::Complete);
}

#[test]
fn pre_cancelled_token_aborts_collection() {
    let temp = tempfile::tempdir().unwrap();
    let paths = paths(&temp);
    let id = instance_id();
    fs::create_dir_all(paths.minecraft_dir(id).join("logs")).unwrap();

    let token = CancellationToken::new();
    token.cancel();
    let error = collection::collect_sources(&paths, id, &DiagnosticSourcePolicy::default(), &token)
        .expect_err("cancelled collection must fail");
    assert_eq!(error.code, ErrorCode::OperationCancelled);
}

#[test]
fn non_regular_source_file_is_skipped() {
    let temp = tempfile::tempdir().unwrap();
    let paths = paths(&temp);
    let id = instance_id();
    let logs = paths.minecraft_dir(id).join("logs");
    fs::create_dir_all(&logs).unwrap();
    fs::create_dir_all(logs.join("debug.log")).unwrap();

    let collected = collection::collect_sources(
        &paths,
        id,
        &DiagnosticSourcePolicy::default(),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(collected.completeness, DiagnosticCompleteness::Partial);
    assert!(collected.text_sources.iter().all(|source| {
        source.path.as_ref().map(|path| path.as_str()) != Some(".minecraft/logs/debug.log")
    }));
}

#[cfg(unix)]
#[test]
fn symlink_escaping_instance_is_rejected() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("secret.log");
    fs::write(&outside_file, format!("token={SECRET_PLACEHOLDER}\n")).unwrap();

    let paths = paths(&temp);
    let id = instance_id();
    let logs = paths.minecraft_dir(id).join("logs");
    fs::create_dir_all(&logs).unwrap();
    symlink(&outside_file, logs.join("latest.log")).unwrap();

    let error = collection::collect_sources(
        &paths,
        id,
        &DiagnosticSourcePolicy::default(),
        &CancellationToken::new(),
    )
    .expect_err("symlink must be rejected");

    assert_eq!(error.code, ErrorCode::DiagnosticSourceUnsafe);
}
