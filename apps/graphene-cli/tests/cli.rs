use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_graphene-cli");
const SENTINEL: &str = "SENTINEL_CURSEFORGE_KEY_DO_NOT_PRINT";

fn command(data_root: &std::path::Path) -> Command {
    let mut command = Command::new(BIN);
    command.arg("--data-root").arg(data_root);
    command
}

fn run(mut command: Command) -> (i32, String, String) {
    let output = command.output().expect("cli runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn usage_error_is_reported_by_clap() {
    let output = Command::new(BIN)
        .arg("not-a-command")
        .output()
        .expect("cli runs");
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stderr.is_empty());
}

#[test]
fn engine_info_json_is_machine_readable() {
    let root = tempfile::tempdir().expect("root");
    let mut command = command(root.path());
    command.args(["--json", "engine", "info"]);
    let (code, stdout, stderr) = run(command);
    assert_eq!(code, 0);
    assert!(stderr.is_empty(), "machine mode emits no stderr: {stderr}");
    assert!(!stdout.contains('\u{1b}'), "no ANSI control sequences");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(value["ok"], serde_json::Value::Bool(true));
    assert!(value["data"]["data_root"].is_string());
    assert!(value["data"]["os"].is_string());
}

#[test]
fn secrets_from_environment_are_never_printed() {
    let root = tempfile::tempdir().expect("root");
    let mut command = command(root.path());
    command
        .args(["-vv", "--json", "engine", "info"])
        .env("GRAPHENE_CURSEFORGE_API_KEY", SENTINEL);
    let (code, stdout, stderr) = run(command);
    assert_eq!(code, 0);
    assert!(!stdout.contains(SENTINEL));
    assert!(!stderr.contains(SENTINEL));
}

#[test]
fn offline_account_round_trip() {
    let root = tempfile::tempdir().expect("root");

    let mut add = command(root.path());
    add.args(["account", "offline-add", "--name", "Alex"]);
    let (code, stdout, _) = run(add);
    assert_eq!(code, 0);
    assert!(stdout.contains("Alex"));

    let mut list = command(root.path());
    list.args(["--json", "account", "list"]);
    let (code, stdout, _) = run(list);
    assert_eq!(code, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let accounts = value["data"].as_array().expect("array");
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0]["profile"]["display_name"], "Alex");
}

#[test]
fn synthetic_operation_emits_structured_events() {
    let root = tempfile::tempdir().expect("root");
    let mut command = command(root.path());
    command.args([
        "--json",
        "engine",
        "synthetic",
        "--steps",
        "3",
        "--delay-ms",
        "1",
    ]);
    let (code, stdout, _) = run(command);
    assert_eq!(code, 0);
    assert!(!stdout.contains('\u{1b}'));
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let events = value["data"]["events"].as_array().expect("events array");
    assert!(!events.is_empty());
}

#[test]
fn invalid_instance_id_returns_safe_error_envelope() {
    let root = tempfile::tempdir().expect("root");
    let mut command = command(root.path());
    command.args(["--json", "instance", "get", "not-a-uuid"]);
    let (code, stdout, stderr) = run(command);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    let value: serde_json::Value = serde_json::from_str(&stderr).expect("valid JSON error");
    assert_eq!(value["ok"], serde_json::Value::Bool(false));
    assert_eq!(value["error"]["code"], "CONFIG_INVALID");
}

#[cfg(unix)]
#[test]
fn interrupt_cancels_active_operation_with_exit_130() {
    use std::process::Stdio;
    use std::time::Duration;

    let root = tempfile::tempdir().expect("root");

    // The handler is installed at process startup, but on a heavily loaded runner an early SIGINT
    // can still land before it is armed and terminate by signal. Retry with a growing delay and
    // accept only a graceful 130; do not depend on progress output being visible.
    for delay_seconds in [1_u64, 3, 6, 10, 15] {
        let mut child = command(root.path())
            .args([
                "engine",
                "synthetic",
                "--steps",
                "10000",
                "--delay-ms",
                "20",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn cli");

        std::thread::sleep(Duration::from_secs(delay_seconds));

        // SAFETY: the child pid is valid for the lifetime of this test.
        let signal = unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
        assert_eq!(signal, 0, "SIGINT delivered");

        let mut status = None;
        for _ in 0..400 {
            if let Some(current) = child.try_wait().expect("try_wait") {
                status = Some(current);
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let status = status.unwrap_or_else(|| {
            let _ = child.kill();
            panic!("cli did not exit after interrupt");
        });

        if status.code() == Some(130) {
            return;
        }
        // Otherwise the signal raced startup and the process terminated by signal; retry later.
    }

    panic!("cli never reported graceful interrupt exit 130");
}
