//! Unified operation lifecycle, progress, event, and cancellation foundation.
//!
//! Terminal states are immutable. Cancellation is requested before state mutation so a concurrent
//! success attempt observes cancellation and resolves to `Cancelled` rather than producing a false
//! success. Child cancellation tokens are linked to their parent token.

mod cancellation;
mod event;
mod progress;
mod state;

pub use cancellation::{CancellationFuture, CancellationToken};
pub use event::{EventNextFuture, OperationEvent, OperationEventKind, OperationEventStream};
pub use progress::Progress;
pub use state::OperationState;

use crate::{ErrorCode, ErrorKind, ErrorSummary, GrapheneError, OperationId, Result};
use event::{Subscriber, terminal_event_kind};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Condvar, Mutex, Weak},
    task::{Context, Poll, Waker},
};

/// Terminal result retained independently of event delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationResult {
    Succeeded,
    Failed { error: ErrorSummary },
    Cancelled,
}

/// Queryable current operation state retained for late subscribers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationSnapshot {
    pub id: OperationId,
    pub parent_id: Option<OperationId>,
    pub name: String,
    pub state: OperationState,
    pub stage: Option<String>,
    pub progress: Progress,
    pub terminal: Option<OperationResult>,
}

#[derive(Debug)]
struct MutableOperation {
    snapshot: OperationSnapshot,
    sequence: u64,
}

#[derive(Debug)]
struct OperationRecord {
    state: Mutex<MutableOperation>,
    cancellation: CancellationToken,
    event_capacity: usize,
    subscribers: Mutex<Vec<Weak<Subscriber>>>,
    published_sequence: Mutex<u64>,
    publication_ready: Condvar,
    terminal_waiters: Mutex<Vec<Waker>>,
}

impl OperationRecord {
    fn publish(&self, event: OperationEvent) {
        // Event generation is protected by the operation state lock, but publishers may race after
        // releasing it. Serialize by sequence so subscribers can never observe N+1 before N.
        let mut published = self
            .published_sequence
            .lock()
            .expect("publication sequence lock poisoned");
        while event.sequence != published.saturating_add(1) {
            published = self
                .publication_ready
                .wait(published)
                .expect("publication sequence lock poisoned");
        }

        let subscribers = {
            let mut subscribers = self.subscribers.lock().expect("subscriber lock poisoned");
            subscribers.retain(|subscriber| subscriber.strong_count() > 0);
            subscribers
                .iter()
                .filter_map(Weak::upgrade)
                .collect::<Vec<_>>()
        };
        for subscriber in subscribers {
            subscriber.push(event.clone());
        }
        *published = event.sequence;
        self.publication_ready.notify_all();
    }

    fn next_event(&self, state: &mut MutableOperation, kind: OperationEventKind) -> OperationEvent {
        state.sequence += 1;
        OperationEvent {
            operation_id: state.snapshot.id,
            parent_id: state.snapshot.parent_id,
            sequence: state.sequence,
            kind,
        }
    }

    fn wake_terminal_waiters(&self) {
        let waiters = std::mem::take(
            &mut *self
                .terminal_waiters
                .lock()
                .expect("terminal waiter lock poisoned"),
        );
        for waiter in waiters {
            waiter.wake();
        }
    }
}

/// Registry that creates and tracks live operation records for one Graphene engine.
#[derive(Debug, Clone)]
pub struct OperationRegistry {
    records: Arc<Mutex<HashMap<OperationId, Weak<OperationRecord>>>>,
    event_capacity: usize,
}

impl OperationRegistry {
    /// Creates a registry with a bounded per-subscription event capacity.
    pub fn new(event_capacity: usize) -> Result<Self> {
        if !(4..=65_536).contains(&event_capacity) {
            return Err(GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                "event channel capacity must be between 4 and 65536",
            )
            .with_context("event_capacity", event_capacity.to_string()));
        }
        Ok(Self {
            records: Arc::new(Mutex::new(HashMap::new())),
            event_capacity,
        })
    }

    /// Creates an independent root operation.
    #[must_use]
    pub fn create(&self, name: impl Into<String>) -> OperationController {
        self.create_inner(name.into(), None)
    }

    /// Creates a child operation linked to the parent's cancellation token.
    #[must_use]
    pub fn create_child(
        &self,
        parent: &OperationHandle,
        name: impl Into<String>,
    ) -> OperationController {
        self.create_inner(name.into(), Some(parent.clone()))
    }

    fn create_inner(&self, name: String, parent: Option<OperationHandle>) -> OperationController {
        let id = OperationId::new();
        let parent_id = parent.as_ref().map(OperationHandle::id);
        let cancellation = parent
            .as_ref()
            .map_or_else(CancellationToken::new, |parent| {
                parent.record.cancellation.child_token()
            });
        let record = Arc::new(OperationRecord {
            state: Mutex::new(MutableOperation {
                snapshot: OperationSnapshot {
                    id,
                    parent_id,
                    name: name.clone(),
                    state: OperationState::Created,
                    stage: None,
                    progress: Progress::Indeterminate,
                    terminal: None,
                },
                sequence: 0,
            }),
            cancellation,
            event_capacity: self.event_capacity,
            subscribers: Mutex::new(Vec::new()),
            published_sequence: Mutex::new(0),
            publication_ready: Condvar::new(),
            terminal_waiters: Mutex::new(Vec::new()),
        });
        self.records
            .lock()
            .expect("operation registry poisoned")
            .insert(id, Arc::downgrade(&record));
        let controller = OperationController { record };
        controller.publish(OperationEventKind::Created { name });
        controller
    }

    /// Looks up an operation by strong ID.
    #[must_use]
    pub fn get(&self, id: OperationId) -> Option<OperationHandle> {
        let mut records = self.records.lock().expect("operation registry poisoned");
        let record = records.get(&id).and_then(Weak::upgrade);
        if record.is_none() {
            records.remove(&id);
        }
        record.map(|record| OperationHandle { record })
    }
}

/// Host-facing operation handle. Mutation is limited to cooperative cancellation.
#[derive(Debug, Clone)]
pub struct OperationHandle {
    record: Arc<OperationRecord>,
}

impl OperationHandle {
    /// Returns this operation's ID.
    #[must_use]
    pub fn id(&self) -> OperationId {
        self.record
            .state
            .lock()
            .expect("state lock poisoned")
            .snapshot
            .id
    }

    /// Returns the parent operation ID, if any.
    #[must_use]
    pub fn parent_id(&self) -> Option<OperationId> {
        self.record
            .state
            .lock()
            .expect("state lock poisoned")
            .snapshot
            .parent_id
    }

    /// Returns a clone of the current retained state.
    #[must_use]
    pub fn snapshot(&self) -> OperationSnapshot {
        self.record
            .state
            .lock()
            .expect("state lock poisoned")
            .snapshot
            .clone()
    }

    /// Returns the current lifecycle state.
    #[must_use]
    pub fn current_state(&self) -> OperationState {
        self.snapshot().state
    }

    /// Requests cancellation. The operation implementation observes the same token at checkpoints.
    pub fn cancel(&self) {
        let (events, became_terminal) = {
            let mut state = self.record.state.lock().expect("state lock poisoned");
            if state.snapshot.state.is_terminal() {
                return;
            }

            // The state lock establishes one ordering point between host cancellation and terminal
            // success. Success seals the token while holding this same lock; therefore either
            // cancellation wins first, or success becomes terminal before cancellation can mutate
            // the token. A successful terminal operation can never carry a late cancelled token.
            self.record.cancellation.cancel();
            if !self.record.cancellation.is_cancelled() {
                return;
            }

            match state.snapshot.state {
                OperationState::Created | OperationState::Queued => {
                    (cancel_terminal_events(&self.record, &mut state), true)
                }
                OperationState::Running => {
                    state.snapshot.state = OperationState::Cancelling;
                    let event = self.record.next_event(
                        &mut state,
                        OperationEventKind::StateChanged {
                            state: OperationState::Cancelling,
                        },
                    );
                    (vec![event], false)
                }
                OperationState::Cancelling => (Vec::new(), false),
                OperationState::Succeeded | OperationState::Failed | OperationState::Cancelled => {
                    (Vec::new(), false)
                }
            }
        };

        for event in events {
            self.record.publish(event);
        }
        if became_terminal {
            self.record.wake_terminal_waiters();
        }
    }

    /// Returns whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.record.cancellation.is_cancelled()
    }

    /// Returns a cloneable cancellation token for implementation checkpoints.
    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.record.cancellation.clone()
    }

    /// Subscribes to events. A late subscription to a terminal operation receives the retained
    /// terminal event immediately; current state is always available through [`Self::snapshot`].
    #[must_use]
    pub fn subscribe(&self) -> OperationEventStream {
        let subscriber = Subscriber::new(self.record.event_capacity);
        let state = self.record.state.lock().expect("state lock poisoned");
        if let Some(terminal) = state.snapshot.terminal.as_ref() {
            subscriber.push(OperationEvent {
                operation_id: state.snapshot.id,
                parent_id: state.snapshot.parent_id,
                sequence: state.sequence,
                kind: terminal_event_kind(terminal),
            });
        } else {
            self.record
                .subscribers
                .lock()
                .expect("subscriber lock poisoned")
                .push(Arc::downgrade(&subscriber));
        }
        drop(state);
        OperationEventStream::new(subscriber)
    }

    /// Waits asynchronously for the retained terminal result.
    pub fn await_result(&self) -> OperationWaitFuture {
        OperationWaitFuture {
            record: Arc::clone(&self.record),
        }
    }
}

/// Internal/service-facing controller for a Graphene operation.
#[derive(Debug, Clone)]
pub struct OperationController {
    record: Arc<OperationRecord>,
}

impl OperationController {
    /// Returns the host-facing handle.
    #[must_use]
    pub fn handle(&self) -> OperationHandle {
        OperationHandle {
            record: Arc::clone(&self.record),
        }
    }

    fn publish(&self, kind: OperationEventKind) {
        let event = {
            let mut state = self.record.state.lock().expect("state lock poisoned");
            self.record.next_event(&mut state, kind)
        };
        self.record.publish(event);
    }

    fn transition(&self, next: OperationState) -> Result<()> {
        let event = {
            let mut state = self.record.state.lock().expect("state lock poisoned");
            if !state.snapshot.state.can_transition_to(next) {
                return Err(invalid_transition(state.snapshot.state, next));
            }
            state.snapshot.state = next;
            self.record
                .next_event(&mut state, OperationEventKind::StateChanged { state: next })
        };
        self.record.publish(event);
        Ok(())
    }

    /// Marks a created operation as queued.
    pub fn queue(&self) -> Result<()> {
        self.transition(OperationState::Queued)
    }

    /// Marks a created/queued operation as running. Cancellation requested before the start resolves
    /// directly to `Cancelled`.
    pub fn start(&self) -> Result<()> {
        let (events, cancelled) = {
            let mut state = self.record.state.lock().expect("state lock poisoned");
            if state.snapshot.state == OperationState::Cancelled {
                (Vec::new(), true)
            } else if self.record.cancellation.is_cancelled() {
                (cancel_terminal_events(&self.record, &mut state), true)
            } else {
                if !state
                    .snapshot
                    .state
                    .can_transition_to(OperationState::Running)
                {
                    return Err(invalid_transition(
                        state.snapshot.state,
                        OperationState::Running,
                    ));
                }
                state.snapshot.state = OperationState::Running;
                let event = self.record.next_event(
                    &mut state,
                    OperationEventKind::StateChanged {
                        state: OperationState::Running,
                    },
                );
                (vec![event], false)
            }
        };
        for event in events {
            self.record.publish(event);
        }
        if cancelled {
            self.record.wake_terminal_waiters();
            Err(cancelled_error())
        } else {
            Ok(())
        }
    }

    /// Changes stage and resets progress to indeterminate for the new stage.
    pub fn set_stage(&self, stage: impl Into<String>) -> Result<()> {
        let stage = stage.into();
        let event = {
            let mut state = self.record.state.lock().expect("state lock poisoned");
            if !matches!(
                state.snapshot.state,
                OperationState::Running | OperationState::Cancelling
            ) {
                return Err(invalid_active_mutation(state.snapshot.state));
            }
            state.snapshot.stage = Some(stage.clone());
            state.snapshot.progress = Progress::Indeterminate;
            self.record
                .next_event(&mut state, OperationEventKind::StageChanged { stage })
        };
        self.record.publish(event);
        Ok(())
    }

    /// Publishes validated monotonic progress within the current stage.
    pub fn set_progress(&self, progress: Progress) -> Result<()> {
        progress.validate()?;
        let event = {
            let mut state = self.record.state.lock().expect("state lock poisoned");
            if !matches!(
                state.snapshot.state,
                OperationState::Running | OperationState::Cancelling
            ) {
                return Err(invalid_active_mutation(state.snapshot.state));
            }
            if !state.snapshot.progress.is_monotonic_to(&progress) {
                return Err(GrapheneError::new(
                    ErrorCode::InternalInvariantViolation,
                    ErrorKind::Internal,
                    "operation progress regressed within a stage",
                ));
            }
            state.snapshot.progress = progress.clone();
            self.record
                .next_event(&mut state, OperationEventKind::Progress { progress })
        };
        self.record.publish(event);
        Ok(())
    }

    /// Completes successfully unless cancellation already won the terminal-state race.
    ///
    /// Success atomically seals cancellation while the operation state is locked. A cancellation
    /// request that wins first produces `Cancelled`; a later request observes the successful
    /// terminal state and cannot change the token or state.
    pub fn succeed(&self) -> Result<OperationResult> {
        let (actual, events) = {
            let mut state = self.record.state.lock().expect("state lock poisoned");
            if let Some(existing) = &state.snapshot.terminal {
                return Ok(existing.clone());
            }

            match state.snapshot.state {
                OperationState::Running if self.record.cancellation.try_seal() => {
                    let result = OperationResult::Succeeded;
                    let events = terminal_events(&self.record, &mut state, result.clone());
                    (result, events)
                }
                OperationState::Running | OperationState::Cancelling
                    if self.record.cancellation.is_cancelled() =>
                {
                    let events = cancel_terminal_events(&self.record, &mut state);
                    (OperationResult::Cancelled, events)
                }
                current => {
                    return Err(invalid_transition(current, OperationState::Succeeded));
                }
            }
        };
        for event in events {
            self.record.publish(event);
        }
        self.record.wake_terminal_waiters();
        Ok(actual)
    }

    /// Completes as failed. A cancelling operation may fail when work or cleanup fails.
    pub fn fail(&self, error: ErrorSummary) -> Result<OperationResult> {
        let result = OperationResult::Failed { error };
        let events = {
            let mut state = self.record.state.lock().expect("state lock poisoned");
            if let Some(existing) = &state.snapshot.terminal {
                return Ok(existing.clone());
            }
            if !matches!(
                state.snapshot.state,
                OperationState::Running | OperationState::Cancelling
            ) {
                return Err(invalid_transition(
                    state.snapshot.state,
                    OperationState::Failed,
                ));
            }
            terminal_events(&self.record, &mut state, result.clone())
        };
        for event in events {
            self.record.publish(event);
        }
        self.record.wake_terminal_waiters();
        Ok(result)
    }

    /// Completes as cancelled. Repeated calls return the retained terminal result.
    pub fn cancelled(&self) -> Result<OperationResult> {
        let (result, events, became_terminal) = {
            let mut state = self.record.state.lock().expect("state lock poisoned");
            if let Some(existing) = &state.snapshot.terminal {
                return Ok(existing.clone());
            }
            self.record.cancellation.cancel();
            if !self.record.cancellation.is_cancelled() {
                return Err(GrapheneError::new(
                    ErrorCode::OperationStateInvalid,
                    ErrorKind::Internal,
                    "cancellation was requested after the operation point of no return",
                ));
            }
            match state.snapshot.state {
                OperationState::Created
                | OperationState::Queued
                | OperationState::Running
                | OperationState::Cancelling => {
                    let events = cancel_terminal_events(&self.record, &mut state);
                    (OperationResult::Cancelled, events, true)
                }
                OperationState::Succeeded | OperationState::Failed | OperationState::Cancelled => {
                    unreachable!("terminal result handled above")
                }
            }
        };
        for event in events {
            self.record.publish(event);
        }
        if became_terminal {
            self.record.wake_terminal_waiters();
        }
        Ok(result)
    }

    /// Seals cancellation immediately before a tiny non-interruptible safe commit. If this
    /// returns `false`, cancellation already won and the commit must not begin.
    #[must_use]
    pub fn seal_cancellation(&self) -> bool {
        self.record.cancellation.try_seal()
    }

    /// Returns whether cancellation is requested at a cooperative checkpoint.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.record.cancellation.is_cancelled()
    }

    /// Returns the cancellation token used by I/O loops.
    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.record.cancellation.clone()
    }
}

fn terminal_events(
    record: &OperationRecord,
    state: &mut MutableOperation,
    result: OperationResult,
) -> Vec<OperationEvent> {
    let terminal_state = match &result {
        OperationResult::Succeeded => OperationState::Succeeded,
        OperationResult::Failed { .. } => OperationState::Failed,
        OperationResult::Cancelled => OperationState::Cancelled,
    };
    state.snapshot.state = terminal_state;
    state.snapshot.terminal = Some(result.clone());
    vec![
        record.next_event(
            state,
            OperationEventKind::StateChanged {
                state: terminal_state,
            },
        ),
        record.next_event(state, terminal_event_kind(&result)),
    ]
}

fn cancel_terminal_events(
    record: &OperationRecord,
    state: &mut MutableOperation,
) -> Vec<OperationEvent> {
    let mut events = Vec::with_capacity(3);
    if state.snapshot.state == OperationState::Running {
        state.snapshot.state = OperationState::Cancelling;
        events.push(record.next_event(
            state,
            OperationEventKind::StateChanged {
                state: OperationState::Cancelling,
            },
        ));
    }
    events.extend(terminal_events(record, state, OperationResult::Cancelled));
    events
}

fn invalid_transition(from: OperationState, to: OperationState) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::OperationStateInvalid,
        ErrorKind::Internal,
        "invalid operation state transition",
    )
    .with_context("from", format!("{from:?}"))
    .with_context("to", format!("{to:?}"))
}

fn cancelled_error() -> GrapheneError {
    GrapheneError::new(
        ErrorCode::OperationCancelled,
        ErrorKind::Cancelled,
        "operation was cancelled",
    )
}

fn invalid_active_mutation(state: OperationState) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::OperationStateInvalid,
        ErrorKind::Internal,
        "operation stage/progress can only change while active",
    )
    .with_context("state", format!("{state:?}"))
}

/// Runtime-independent future for a retained terminal result.
pub struct OperationWaitFuture {
    record: Arc<OperationRecord>,
}

impl Future for OperationWaitFuture {
    type Output = OperationResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(result) = self
            .record
            .state
            .lock()
            .expect("state lock poisoned")
            .snapshot
            .terminal
            .clone()
        {
            return Poll::Ready(result);
        }
        let mut waiters = self
            .record
            .terminal_waiters
            .lock()
            .expect("terminal waiter lock poisoned");
        if let Some(result) = self
            .record
            .state
            .lock()
            .expect("state lock poisoned")
            .snapshot
            .terminal
            .clone()
        {
            return Poll::Ready(result);
        }
        if !waiters.iter().any(|waker| waker.will_wake(cx.waker())) {
            waiters.push(cx.waker().clone());
        }
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[test]
    fn parent_child_relation_and_cancellation_are_preserved() {
        let registry = OperationRegistry::new(16).expect("valid registry");
        let parent = registry.create("parent");
        parent.start().expect("parent starts");
        let child = registry.create_child(&parent.handle(), "child");
        child.start().expect("child starts");
        assert_eq!(child.handle().parent_id(), Some(parent.handle().id()));
        parent.handle().cancel();
        assert!(child.handle().is_cancelled());
        assert_eq!(
            child.cancelled().expect("cancel child"),
            OperationResult::Cancelled
        );
    }

    #[test]
    fn inherited_cancellation_before_start_becomes_terminal_cancelled() {
        let registry = OperationRegistry::new(16).expect("valid registry");
        let parent = registry.create("parent");
        let child = registry.create_child(&parent.handle(), "child");
        parent.handle().cancel();

        let error = child.start().expect_err("cancelled child cannot start");
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(child.handle().current_state(), OperationState::Cancelled);
        assert_eq!(
            child.handle().snapshot().terminal,
            Some(OperationResult::Cancelled)
        );
    }

    #[test]
    fn cancellation_cannot_race_into_success() {
        for _ in 0..128 {
            let registry = OperationRegistry::new(16).expect("valid registry");
            let controller = registry.create("race");
            controller.start().expect("starts");
            let handle = controller.handle();
            let controller = Arc::new(controller);
            let barrier = Arc::new(Barrier::new(3));

            let cancel_barrier = Arc::clone(&barrier);
            let cancel_handle = handle.clone();
            let cancel = std::thread::spawn(move || {
                cancel_barrier.wait();
                cancel_handle.cancel();
            });

            let success_barrier = Arc::clone(&barrier);
            let success_controller = Arc::clone(&controller);
            let success = std::thread::spawn(move || {
                success_barrier.wait();
                success_controller.succeed().expect("terminal transition")
            });

            barrier.wait();
            cancel.join().expect("cancel thread");
            let _ = success.join().expect("success thread");
            let state = handle.current_state();
            assert!(matches!(
                state,
                OperationState::Succeeded | OperationState::Cancelled
            ));
            assert_ne!(
                (state, handle.is_cancelled()),
                (OperationState::Succeeded, true)
            );
            if handle.is_cancelled() {
                assert_eq!(state, OperationState::Cancelled);
            }
            assert!(state.is_terminal());
        }
    }

    #[test]
    fn cancellation_that_wins_before_success_completes_as_cancelled() {
        let registry = OperationRegistry::new(8).expect("valid registry");
        let operation = registry.create("cancel-before-success");
        operation.start().expect("starts");
        operation.handle().cancel();

        assert_eq!(
            operation.succeed().expect("terminal transition"),
            OperationResult::Cancelled
        );
        assert_eq!(
            operation.handle().current_state(),
            OperationState::Cancelled
        );
    }

    #[test]
    fn success_before_running_is_rejected_without_terminal_mutation() {
        let registry = OperationRegistry::new(8).expect("valid registry");
        let operation = registry.create("invalid-success");
        let error = operation.succeed().expect_err("created cannot succeed");
        assert_eq!(error.code, ErrorCode::OperationStateInvalid);
        assert_eq!(operation.handle().current_state(), OperationState::Created);
    }

    #[test]
    fn terminal_event_is_emitted_at_most_once() {
        let registry = OperationRegistry::new(8).expect("valid registry");
        let operation = registry.create("terminal");
        let stream = operation.handle().subscribe();
        operation.start().expect("starts");
        assert_eq!(
            operation.succeed().expect("succeed"),
            OperationResult::Succeeded
        );
        assert_eq!(
            operation.succeed().expect("succeed"),
            OperationResult::Succeeded
        );
        operation.handle().cancel();
        let mut terminal_events = 0;
        while let Some(event) = stream.try_next() {
            if matches!(
                event.kind,
                OperationEventKind::Completed
                    | OperationEventKind::Failed { .. }
                    | OperationEventKind::Cancelled
            ) {
                terminal_events += 1;
            }
        }
        assert_eq!(terminal_events, 1);
    }

    #[test]
    fn stage_and_progress_require_an_active_operation() {
        let registry = OperationRegistry::new(8).expect("valid registry");
        let operation = registry.create("inactive");
        assert_eq!(
            operation
                .set_stage("too-early")
                .expect_err("created stage mutation must fail")
                .code,
            ErrorCode::OperationStateInvalid
        );
        assert_eq!(
            operation
                .set_progress(Progress::Items {
                    completed: 0,
                    total: Some(1),
                })
                .expect_err("created progress mutation must fail")
                .code,
            ErrorCode::OperationStateInvalid
        );
    }

    #[test]
    fn late_subscriber_receives_retained_terminal_event() {
        let registry = OperationRegistry::new(8).expect("valid registry");
        let operation = registry.create("late");
        operation.start().expect("starts");
        operation.succeed().expect("succeeds");

        let stream = operation.handle().subscribe();
        let event = stream.try_next().expect("retained terminal event");
        assert!(matches!(event.kind, OperationEventKind::Completed));
        assert!(stream.try_next().is_none());
    }
}
