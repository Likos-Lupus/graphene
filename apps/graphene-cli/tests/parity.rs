//! Cross-host parity contract.
//!
//! The Tauri adapter asserts the same normalized account contract in its own unit tests. Both hosts
//! derive it from the same Graphene engine, so equivalent fixture semantics must produce equivalent
//! normalized outcomes regardless of presentation.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_graphene-cli");

fn normalized(account: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "display_name": account["profile"]["display_name"],
        "kind": account["kind"],
        "state": account["state"],
    })
}

#[test]
fn cli_account_outcome_matches_the_shared_host_contract() {
    let root = tempfile::tempdir().expect("root");

    let add = Command::new(BIN)
        .arg("--data-root")
        .arg(root.path())
        .args(["account", "offline-add", "--name", "Parity"])
        .output()
        .expect("add runs");
    assert_eq!(add.status.code(), Some(0));

    let list = Command::new(BIN)
        .arg("--data-root")
        .arg(root.path())
        .args(["--json", "account", "list"])
        .output()
        .expect("list runs");
    assert_eq!(list.status.code(), Some(0));

    let value: serde_json::Value =
        serde_json::from_slice(&list.stdout).expect("machine-readable JSON");
    let accounts = value["data"].as_array().expect("account array");
    assert_eq!(accounts.len(), 1);

    assert_eq!(
        normalized(&accounts[0]),
        serde_json::json!({
            "display_name": "Parity",
            "kind": "offline",
            "state": "ready",
        })
    );
}
