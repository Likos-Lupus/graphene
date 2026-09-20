use graphene::{
    ErrorCode, ErrorKind, GameEventStream, GameExit, GrapheneError, RefreshCredential,
    SecretRecordIdentity, SecretStore,
};
use graphene_reference_host_support::{
    HostConfig, HostOperationRegistry, HostOperationTerminal, HostRun, HostRunRegistry,
    resolve_data_root,
};
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Default)]
struct FakeSecretStore {
    records: Mutex<HashMap<String, String>>,
}

impl SecretStore for FakeSecretStore {
    fn get(
        &self,
        identity: &SecretRecordIdentity,
    ) -> Result<Option<RefreshCredential>, GrapheneError> {
        Ok(self
            .records
            .lock()
            .expect("fake store poisoned")
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
            .expect("fake store poisoned")
            .insert(identity.key(), value.expose_secret().to_owned());
        Ok(())
    }

    fn delete(&self, identity: &SecretRecordIdentity) -> Result<(), GrapheneError> {
        self.records
            .lock()
            .expect("fake store poisoned")
            .remove(&identity.key());
        Ok(())
    }
}

struct FakeRun {
    pid: u32,
    killed: AtomicBool,
    terminal: tokio::sync::Mutex<Option<GameExit>>,
    notify: tokio::sync::Notify,
}

impl FakeRun {
    fn new(pid: u32) -> Self {
        Self {
            pid,
            killed: AtomicBool::new(false),
            terminal: tokio::sync::Mutex::new(None),
            notify: tokio::sync::Notify::new(),
        }
    }
}

impl HostRun for FakeRun {
    fn pid(&self) -> u32 {
        self.pid
    }

    fn dropped_output_count(&self) -> u64 {
        0
    }

    fn take_events(&self) -> Option<GameEventStream> {
        None
    }

    fn wait(&self) -> Pin<Box<dyn Future<Output = Result<GameExit, GrapheneError>> + Send + '_>> {
        Box::pin(async move {
            loop {
                let notified = self.notify.notified();
                if let Some(exit) = *self.terminal.lock().await {
                    return Ok(exit);
                }
                notified.await;
            }
        })
    }

    fn kill(&self) -> Pin<Box<dyn Future<Output = Result<(), GrapheneError>> + Send + '_>> {
        Box::pin(async move {
            self.killed.store(true, Ordering::SeqCst);
            *self.terminal.lock().await = Some(GameExit {
                success: false,
                code: None,
                killed: true,
            });
            self.notify.notify_waiters();
            Ok(())
        })
    }
}

async fn build_graphene(data_root: &std::path::Path) -> graphene::Graphene {
    HostConfig::new(data_root)
        .build(Arc::new(FakeSecretStore::default()))
        .await
        .expect("engine builds from explicit host configuration")
}

#[test]
fn explicit_data_root_wins_over_environment() {
    let explicit = std::path::PathBuf::from("/tmp/graphene-explicit-root");
    assert_eq!(resolve_data_root(Some(explicit.clone())).unwrap(), explicit);
}

#[test]
fn default_data_root_uses_environment_when_present() {
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().expect("env lock poisoned");
    let previous = std::env::var_os("GRAPHENE_DATA_ROOT");
    // SAFETY: this test holds a process-wide lock covering the only env mutation in this file.
    unsafe { std::env::set_var("GRAPHENE_DATA_ROOT", "/tmp/graphene-env-root") };
    let resolved = resolve_data_root(None).unwrap();
    if let Some(previous) = previous {
        // SAFETY: guarded by the same lock as the mutation above.
        unsafe { std::env::set_var("GRAPHENE_DATA_ROOT", previous) };
    } else {
        // SAFETY: guarded by the same lock as the mutation above.
        unsafe { std::env::remove_var("GRAPHENE_DATA_ROOT") };
    }
    assert_eq!(resolved, std::path::PathBuf::from("/tmp/graphene-env-root"));
}

#[tokio::test]
async fn two_hosts_use_independent_data_roots_without_a_global_singleton() {
    let first_root = tempfile::tempdir().expect("first root");
    let second_root = tempfile::tempdir().expect("second root");
    let first = build_graphene(first_root.path()).await;
    let second = build_graphene(second_root.path()).await;

    assert_ne!(first.data_root(), second.data_root());
    // The engine canonicalizes the resolved data root, so compare against the canonical temp path
    // (macOS temp directories resolve through the `/private` symlink).
    assert_eq!(
        first.data_root(),
        first_root
            .path()
            .canonicalize()
            .expect("first canonical root")
    );
    assert_eq!(
        second.data_root(),
        second_root
            .path()
            .canonicalize()
            .expect("second canonical root")
    );
}

#[tokio::test]
async fn operation_registry_reports_terminal_state_and_events() {
    let root = tempfile::tempdir().expect("root");
    let engine = build_graphene(root.path()).await;
    let registry = HostOperationRegistry::new(16, Duration::from_millis(50));

    let synthetic = engine
        .operations()
        .synthetic(None, 3, Duration::from_millis(5));
    let handle = synthetic.operation();
    let id = registry.register(handle);
    let mut events = registry.subscribe(id).expect("subscription");

    synthetic.await_result().await.expect("synthetic succeeds");

    let mut forwarded = 0_u64;
    while let Ok(event) = events.try_recv() {
        forwarded += 1;
        assert_eq!(event.operation_id, id);
    }
    assert!(forwarded > 0, "at least one event is forwarded");

    let terminal = wait_for_terminal(&registry, id).await;
    assert_eq!(terminal, HostOperationTerminal::Succeeded);
}

#[tokio::test]
async fn operation_registry_cancellation_is_cooperative_and_reported() {
    let root = tempfile::tempdir().expect("root");
    let engine = build_graphene(root.path()).await;
    let registry = HostOperationRegistry::new(16, Duration::from_millis(50));

    let synthetic = engine
        .operations()
        .synthetic(None, 50, Duration::from_millis(10));
    let id = registry.register(synthetic.operation());
    registry.cancel(id).expect("tracked operation cancels");
    let _ = synthetic.await_result().await;

    let terminal = wait_for_terminal(&registry, id).await;
    assert_eq!(terminal, HostOperationTerminal::Cancelled);
}

#[tokio::test]
async fn unknown_operation_cancellation_is_a_safe_error() {
    let registry = HostOperationRegistry::new(4, Duration::from_millis(10));
    let unknown = graphene::OperationId::new();
    let error = registry.cancel(unknown).expect_err("unknown id fails");
    assert_eq!(error.code, ErrorCode::OperationStateInvalid);
    assert_eq!(error.kind, ErrorKind::Internal);
}

#[tokio::test]
async fn run_registry_lifecycle_is_lease_safe() {
    let registry = HostRunRegistry::new(8, Duration::from_millis(10));
    let run = Arc::new(FakeRun::new(4242));
    let run_id = registry.insert(run);

    let summaries = registry.list();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].run_id, run_id);
    assert_eq!(summaries[0].pid, 4242);

    registry.kill(&run_id).await.expect("kill succeeds");
    let exit = registry.wait(&run_id).await.expect("wait succeeds");
    assert!(exit.killed);
    assert!(registry.remove(&run_id));
    assert!(registry.get(&run_id).is_none());
}

async fn wait_for_terminal(
    registry: &HostOperationRegistry,
    id: graphene::OperationId,
) -> HostOperationTerminal {
    for _ in 0..200 {
        if let Some(terminal) = registry.terminal(id) {
            return terminal;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("operation terminal was not recorded");
}

#[tokio::test]
async fn run_registry_shutdown_terminates_tracked_runs() {
    let registry = HostRunRegistry::new(8, Duration::from_millis(10));
    let run = Arc::new(FakeRun::new(99));
    let run_id = registry.insert(run.clone());

    registry.shutdown().await;

    let exit = registry
        .wait(&run_id)
        .await
        .expect("terminal after shutdown");
    assert!(exit.killed);
    assert!(run.killed.load(Ordering::SeqCst));
    assert!(registry.remove(&run_id));
}

#[tokio::test]
async fn host_build_rejects_a_non_directory_data_root() {
    let directory = tempfile::tempdir().expect("directory");
    let file = directory.path().join("not-a-directory");
    std::fs::write(&file, b"not a data root").expect("file");

    let result = HostConfig::new(&file)
        .build(Arc::new(FakeSecretStore::default()))
        .await;
    assert!(result.is_err());
}
