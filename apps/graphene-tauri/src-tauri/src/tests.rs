use crate::commands;
use crate::dto::{AccountSummary, AuthInteractionView};
use crate::error::HostError;
use crate::state::TauriAppState;
use graphene::{
    ErrorCode, ErrorKind, GrapheneError, RefreshCredential, SecretRecordIdentity, SecretStore,
};
use graphene_reference_host_support::{HostConfig, HostOperationTerminal};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Default)]
struct FakeStore {
    records: Mutex<HashMap<String, String>>,
}

impl SecretStore for FakeStore {
    fn get(
        &self,
        identity: &SecretRecordIdentity,
    ) -> Result<Option<RefreshCredential>, GrapheneError> {
        Ok(self
            .records
            .lock()
            .expect("fake store")
            .get(&identity.key())
            .map(|value| RefreshCredential::new(value.clone())))
    }

    fn put(
        &self,
        identity: &SecretRecordIdentity,
        value: &RefreshCredential,
    ) -> Result<(), GrapheneError> {
        self.records
            .lock()
            .expect("fake store")
            .insert(identity.key(), value.expose_secret().to_owned());
        Ok(())
    }

    fn delete(&self, identity: &SecretRecordIdentity) -> Result<(), GrapheneError> {
        self.records
            .lock()
            .expect("fake store")
            .remove(&identity.key());
        Ok(())
    }
}

async fn build_state(root: &std::path::Path) -> TauriAppState {
    let engine = HostConfig::new(root)
        .build(Arc::new(FakeStore::default()))
        .await
        .expect("engine builds");
    TauriAppState::new(engine)
}

#[tokio::test]
async fn engine_info_reports_the_resolved_data_root() {
    let root = tempfile::tempdir().expect("root");
    let state = build_state(root.path()).await;
    let info = commands::engine::info(&state);

    assert_eq!(info.data_root, root.path().display().to_string());
    assert!(!info.os.is_empty());
}

#[tokio::test]
async fn instance_inventory_starts_empty() {
    let root = tempfile::tempdir().expect("root");
    let state = build_state(root.path()).await;
    let instances = commands::instances::list(&state).await.expect("list");

    assert!(instances.is_empty());
}

#[tokio::test]
async fn offline_account_round_trip_has_no_secret_fields() {
    let root = tempfile::tempdir().expect("root");
    let state = build_state(root.path()).await;

    let created = commands::accounts::offline_add(&state, "Steph")
        .await
        .expect("add");
    assert_eq!(created.display_name, "Steph");

    let accounts = commands::accounts::list(&state).await.expect("list");
    assert_eq!(accounts.len(), 1);

    let value = serde_json::to_value(&accounts[0]).expect("serialize");
    let object = value.as_object().expect("object");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["account_id", "display_name", "kind", "state"]);
}

#[tokio::test]
async fn auth_interaction_serializes_only_intended_fields() {
    let view = AuthInteractionView {
        operation_id: "op".to_owned(),
        kind: "device-authorization".to_owned(),
        verification_uri: "https://example.invalid/device".to_owned(),
        user_code: "CODE".to_owned(),
        expires_in_seconds: 900,
        poll_interval_seconds: 5,
    };
    let value = serde_json::to_value(&view).expect("serialize");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();

    assert_eq!(
        keys,
        vec![
            "expires_in_seconds",
            "kind",
            "operation_id",
            "poll_interval_seconds",
            "user_code",
            "verification_uri",
        ]
    );
}

#[test]
fn host_error_envelope_carries_safe_fields() {
    let error = GrapheneError::new(
        ErrorCode::ConfigInvalid,
        ErrorKind::Configuration,
        "bad input",
    )
    .with_context("field", "value");

    let envelope = HostError::from(error);
    let value = serde_json::to_value(&envelope).expect("serialize");

    assert_eq!(value["code"], "CONFIG_INVALID");
    assert_eq!(value["kind"], "Configuration");
    assert_eq!(value["context"]["field"], "value");
}

#[tokio::test]
async fn synthetic_operation_is_observable_and_cancellable() {
    let root = tempfile::tempdir().expect("root");
    let state = build_state(root.path()).await;

    let id = commands::engine::register_synthetic(&state, 200, 5);
    assert_eq!(commands::operations::list(&state).len(), 1);
    assert!(
        commands::operations::snapshot(&state, &id.to_string())
            .expect("snapshot")
            .is_some()
    );

    commands::operations::cancel(&state, &id.to_string()).expect("cancel");
    let terminal = wait_for_terminal(&state, id).await;
    assert_eq!(terminal, HostOperationTerminal::Cancelled);
}

async fn wait_for_terminal(
    state: &TauriAppState,
    id: graphene::OperationId,
) -> HostOperationTerminal {
    for _ in 0..400 {
        if let Some(terminal) = state.operations().terminal(id) {
            return terminal;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("terminal state was not recorded");
}

#[test]
fn account_summary_view_is_locally_defined() {
    // The conversion target is app-local and cannot accidentally include engine handles.
    fn assert_serializable<T: serde::Serialize>() {}
    assert_serializable::<AccountSummary>();
}
