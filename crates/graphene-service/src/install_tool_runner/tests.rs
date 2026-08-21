use super::*;
use graphene_core::CancellationToken;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("graphene-install-tool-runner-{name}-{unique}"));
    fs::create_dir_all(&path).expect("temp dir");
    path
}

fn compile_fake_java(directory: &Path, name: &str) -> PathBuf {
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/java/fake_java.rs");
    let executable = directory.join(if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    });

    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc)
        .arg(source)
        .arg("-O")
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("compile fake Java");
    assert!(
        output.status.success(),
        "fake Java compilation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    executable
}

fn request(java: PathBuf, working: &Path, arguments: Vec<String>) -> ToolRunRequest {
    ToolRunRequest {
        processor_id: "fixture-processor".to_owned(),
        java: SelectedToolJava {
            executable: java,
            major_version: 21,
        },
        executable_jar: working.join("tool.jar"),
        classpath: vec![working.join("dependency with spaces.jar")],
        main_class: "org.example.Tool".to_owned(),
        arguments,
        working_directory: working.to_path_buf(),
        timeout: Duration::from_secs(5),
        cancellation: CancellationToken::new(),
    }
}

#[tokio::test]
async fn runner_uses_selected_java_direct_argv_and_bounds_invalid_utf8_output() {
    let root = temp_dir("argv");
    let java = compile_fake_java(&root, "fake java with spaces");
    let capture = root.join("captured argv.txt");
    let result = run_tool(request(
        java,
        &root,
        vec![
            format!("--fake-capture={}", capture.display()),
            "argument with spaces".to_owned(),
            "--fake-spam=20000".to_owned(),
        ],
    ))
    .await
    .expect("run fake Java");

    assert_eq!(result.exit_code, Some(0));
    assert!(
        result.stdout.contains('\u{fffd}'),
        "invalid UTF-8 must be lossily bounded"
    );
    assert!(result.stdout_truncated || result.stderr_truncated);
    let captured = fs::read_to_string(&capture).expect("captured argv");
    assert!(captured.lines().any(|line| line == "-cp"));
    assert!(captured.lines().any(|line| line == "org.example.Tool"));
    assert!(captured.lines().any(|line| line == "argument with spaces"));
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn runner_reports_nonzero_spawn_timeout_and_cancellation_structurally() {
    let root = temp_dir("failure");
    let java = compile_fake_java(&root, "fake-java");

    let nonzero = run_tool(request(
        java.clone(),
        &root,
        vec!["--fake-exit=7".to_owned()],
    ))
    .await
    .expect("nonzero is a structured result");
    assert_eq!(nonzero.exit_code, Some(7));

    let mut timed = request(java.clone(), &root, vec!["--fake-sleep".to_owned()]);
    timed.timeout = Duration::from_millis(100);
    assert_eq!(
        run_tool(timed).await.expect_err("timeout").code,
        ErrorCode::LoaderProcessorTimeout
    );

    let cancelled = request(java, &root, vec!["--fake-sleep".to_owned()]);
    let token = cancelled.cancellation.clone();
    let task = tokio::spawn(async move { run_tool(cancelled).await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    token.cancel();
    assert_eq!(
        task.await.expect("join").expect_err("cancelled").code,
        ErrorCode::LoaderProcessorCancelled
    );

    let missing = root.join("missing-java");
    assert_eq!(
        run_tool(request(missing, &root, Vec::new()))
            .await
            .expect_err("spawn failure")
            .code,
        ErrorCode::LoaderProcessorFailed
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn runner_does_not_spawn_when_already_cancelled() {
    let root = temp_dir("pre-cancel");
    let token = CancellationToken::new();
    token.cancel();
    let mut value = request(root.join("does-not-exist"), &root, Vec::new());
    value.cancellation = token;
    assert_eq!(
        run_tool(value)
            .await
            .expect_err("cancelled before spawn")
            .code,
        ErrorCode::LoaderProcessorCancelled
    );
    let _ = fs::remove_dir_all(root);
}
