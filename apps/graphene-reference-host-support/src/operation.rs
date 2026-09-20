use graphene::{
    ErrorCode, ErrorKind, ErrorSummary, GrapheneError, OperationEvent, OperationHandle,
    OperationId, OperationResult, OperationSnapshot, OperationState, Progress,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::broadcast;

/// Cloneable, UI-safe terminal summary retained by the host operation registry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HostOperationTerminal {
    Succeeded,
    Cancelled,
    Failed { error: ErrorSummary },
}

/// Host-facing snapshot of a tracked operation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OperationSummary {
    pub id: OperationId,
    pub name: String,
    pub state: OperationState,
    pub stage: Option<String>,
    pub progress: Progress,
    pub terminal: Option<HostOperationTerminal>,
}

struct Record {
    handle: OperationHandle,
    events: broadcast::Sender<OperationEvent>,
    terminal: Mutex<Option<HostOperationTerminal>>,
}

/// Host-local registry that retains Graphene operation handles, forwards bounded events, and keeps
/// terminal state until a bounded retention window elapses.
///
/// This is host state, never a process-global engine singleton. It depends only on public
/// `graphene` operation types.
#[derive(Clone)]
pub struct HostOperationRegistry {
    records: Arc<Mutex<HashMap<OperationId, Arc<Record>>>>,
    event_capacity: usize,
    retention: Duration,
}

impl HostOperationRegistry {
    #[must_use]
    pub fn new(event_capacity: usize, retention: Duration) -> Self {
        Self {
            records: Arc::new(Mutex::new(HashMap::new())),
            event_capacity: event_capacity.max(1),
            retention,
        }
    }

    /// Starts tracking an operation handle and spawns its bounded event forwarder.
    ///
    /// Must be called from within a Tokio runtime.
    pub fn register(&self, handle: OperationHandle) -> OperationId {
        let id = handle.id();
        let (events, _receiver) = broadcast::channel(self.event_capacity);
        let record = Arc::new(Record {
            handle: handle.clone(),
            events,
            terminal: Mutex::new(None),
        });

        self.records
            .lock()
            .expect("operation registry poisoned")
            .insert(id, Arc::clone(&record));

        let records = Arc::clone(&self.records);
        let retention = self.retention;

        tokio::spawn(async move {
            let stream = handle.subscribe();
            while let Some(event) = stream.next().await {
                let _ = record.events.send(event);
            }

            let terminal = terminal_from(handle.await_result().await);
            *record.terminal.lock().expect("operation terminal poisoned") = Some(terminal);
            tokio::time::sleep(retention).await;
            records
                .lock()
                .expect("operation registry poisoned")
                .remove(&id);
        });

        id
    }

    #[must_use]
    pub fn get(&self, id: OperationId) -> Option<OperationHandle> {
        self.record(id).map(|record| record.handle.clone())
    }

    #[must_use]
    pub fn snapshot(&self, id: OperationId) -> Option<OperationSnapshot> {
        self.record(id).map(|record| record.handle.snapshot())
    }

    #[must_use]
    pub fn terminal(&self, id: OperationId) -> Option<HostOperationTerminal> {
        self.record(id).and_then(|record| {
            record
                .terminal
                .lock()
                .expect("operation terminal poisoned")
                .clone()
        })
    }

    #[must_use]
    pub fn subscribe(&self, id: OperationId) -> Option<broadcast::Receiver<OperationEvent>> {
        self.record(id).map(|record| record.events.subscribe())
    }

    /// Requests cooperative cancellation of a tracked operation.
    pub fn cancel(&self, id: OperationId) -> Result<(), GrapheneError> {
        let record = self.record(id).ok_or_else(|| unknown_operation(id))?;
        record.handle.cancel();
        Ok(())
    }

    pub fn remove(&self, id: OperationId) -> bool {
        self.records
            .lock()
            .expect("operation registry poisoned")
            .remove(&id)
            .is_some()
    }

    #[must_use]
    pub fn list(&self) -> Vec<OperationSummary> {
        let records: Vec<Arc<Record>> = self
            .records
            .lock()
            .expect("operation registry poisoned")
            .values()
            .cloned()
            .collect();
        let mut summaries: Vec<OperationSummary> = records
            .into_iter()
            .map(|record| {
                let snapshot = record.handle.snapshot();
                OperationSummary {
                    terminal: record
                        .terminal
                        .lock()
                        .expect("operation terminal poisoned")
                        .clone(),
                    id: snapshot.id,
                    name: snapshot.name,
                    state: snapshot.state,
                    stage: snapshot.stage,
                    progress: snapshot.progress,
                }
            })
            .collect();
        summaries.sort_by_key(|summary| summary.id.to_string());
        summaries
    }

    fn record(&self, id: OperationId) -> Option<Arc<Record>> {
        self.records
            .lock()
            .expect("operation registry poisoned")
            .get(&id)
            .cloned()
    }
}

fn terminal_from(result: OperationResult) -> HostOperationTerminal {
    match result {
        OperationResult::Succeeded => HostOperationTerminal::Succeeded,
        OperationResult::Cancelled => HostOperationTerminal::Cancelled,
        OperationResult::Failed { error } => HostOperationTerminal::Failed { error },
    }
}

/// Builds the standard safe error for an operation id the host does not track.
#[must_use]
pub fn unknown_operation(id: OperationId) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::OperationStateInvalid,
        ErrorKind::Internal,
        "operation is not tracked by this host",
    )
    .with_context("operation_id", id.to_string())
}
