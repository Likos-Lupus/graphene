use graphene::{
    ErrorCode, ErrorKind, GameEvent, GameEventStream, GameExit, GrapheneError, RunningGame,
};
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::broadcast;

/// Host-local abstraction over a running game process.
///
/// Implemented for the engine's [`RunningGame`], which keeps the instance shared lease alive for
/// the whole process lifetime. A host never touches Tokio child handles or bypasses
/// `RunningGame::kill`/`wait`.
pub trait HostRun: Send + Sync {
    fn pid(&self) -> u32;

    fn dropped_output_count(&self) -> u64;

    fn take_events(&self) -> Option<GameEventStream>;

    fn wait(&self) -> Pin<Box<dyn Future<Output = Result<GameExit, GrapheneError>> + Send + '_>>;

    fn kill(&self) -> Pin<Box<dyn Future<Output = Result<(), GrapheneError>> + Send + '_>>;
}

impl HostRun for RunningGame {
    fn pid(&self) -> u32 {
        RunningGame::pid(self)
    }

    fn dropped_output_count(&self) -> u64 {
        RunningGame::dropped_output_count(self)
    }

    fn take_events(&self) -> Option<GameEventStream> {
        RunningGame::take_events(self)
    }

    fn wait(&self) -> Pin<Box<dyn Future<Output = Result<GameExit, GrapheneError>> + Send + '_>> {
        Box::pin(RunningGame::wait(self))
    }

    fn kill(&self) -> Pin<Box<dyn Future<Output = Result<(), GrapheneError>> + Send + '_>> {
        Box::pin(RunningGame::kill(self))
    }
}

/// Bounded forward envelope correlating a game event with its host run id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunEventEnvelope {
    pub run_id: String,
    pub event: GameEvent,
}

/// Host-facing summary of a tracked run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RunSummary {
    pub run_id: String,
    pub pid: u32,
    pub dropped_output_count: u64,
}

/// Host-local registry of running-game handles keyed by an opaque run id.
///
/// Holding a handle keeps the engine instance lease alive. Callers remove a run only after it has
/// reached a terminal state so the lease is never dropped while the child is alive.
#[derive(Clone)]
pub struct HostRunRegistry {
    runs: Arc<Mutex<HashMap<String, Arc<dyn HostRun>>>>,
    events: broadcast::Sender<RunEventEnvelope>,
    counter: Arc<AtomicU64>,
    retention: Duration,
}

impl HostRunRegistry {
    #[must_use]
    pub fn new(event_capacity: usize, retention: Duration) -> Self {
        let (events, _receiver) = broadcast::channel(event_capacity.max(1));
        Self {
            runs: Arc::new(Mutex::new(HashMap::new())),
            events,
            counter: Arc::new(AtomicU64::new(0)),
            retention,
        }
    }

    /// Retains a run handle, forwards its bounded event stream, and returns the host run id.
    pub fn insert(&self, run: Arc<dyn HostRun>) -> String {
        let run_id = format!("run-{}", self.counter.fetch_add(1, Ordering::Relaxed) + 1);
        self.runs
            .lock()
            .expect("run registry poisoned")
            .insert(run_id.clone(), Arc::clone(&run));

        if let Some(mut stream) = run.take_events() {
            let sender = self.events.clone();
            let id = run_id.clone();
            let runs = Arc::clone(&self.runs);
            let retention = self.retention;
            tokio::spawn(async move {
                while let Some(event) = stream.next().await {
                    let _ = sender.send(RunEventEnvelope {
                        run_id: id.clone(),
                        event,
                    });
                }
                tokio::time::sleep(retention).await;
                runs.lock().expect("run registry poisoned").remove(&id);
            });
        }
        run_id
    }

    #[must_use]
    pub fn get(&self, run_id: &str) -> Option<Arc<dyn HostRun>> {
        self.runs
            .lock()
            .expect("run registry poisoned")
            .get(run_id)
            .cloned()
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<RunEventEnvelope> {
        self.events.subscribe()
    }

    pub async fn kill(&self, run_id: &str) -> Result<(), GrapheneError> {
        let run = self.get(run_id).ok_or_else(|| unknown_run(run_id))?;
        run.kill().await
    }

    pub async fn wait(&self, run_id: &str) -> Result<GameExit, GrapheneError> {
        let run = self.get(run_id).ok_or_else(|| unknown_run(run_id))?;
        run.wait().await
    }

    /// Drops the host reference to a run. Only call after the run reaches a terminal state.
    pub fn remove(&self, run_id: &str) -> bool {
        self.runs
            .lock()
            .expect("run registry poisoned")
            .remove(run_id)
            .is_some()
    }

    /// Explicitly terminates every tracked run; used by host shutdown paths.
    pub async fn shutdown(&self) {
        let runs: Vec<Arc<dyn HostRun>> = self
            .runs
            .lock()
            .expect("run registry poisoned")
            .values()
            .cloned()
            .collect();

        for run in runs {
            let _ = run.kill().await;
        }
    }

    #[must_use]
    pub fn list(&self) -> Vec<RunSummary> {
        let runs: Vec<(String, Arc<dyn HostRun>)> = self
            .runs
            .lock()
            .expect("run registry poisoned")
            .iter()
            .map(|(id, run)| (id.clone(), Arc::clone(run)))
            .collect();
        let mut summaries: Vec<RunSummary> = runs
            .into_iter()
            .map(|(run_id, run)| RunSummary {
                run_id,
                pid: run.pid(),
                dropped_output_count: run.dropped_output_count(),
            })
            .collect();

        summaries.sort_by(|left, right| left.run_id.cmp(&right.run_id));
        summaries
    }
}

fn unknown_run(run_id: &str) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::OperationStateInvalid,
        ErrorKind::Internal,
        "game run is not tracked by this host",
    )
    .with_context("run_id", run_id)
}
