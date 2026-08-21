use super::{OperationResult, OperationState, Progress};
use crate::{ErrorSummary, OperationId};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

/// Typed operation event payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationEvent {
    pub operation_id: OperationId,
    pub parent_id: Option<OperationId>,
    pub sequence: u64,
    pub kind: OperationEventKind,
}

/// Operation event classes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OperationEventKind {
    Created { name: String },
    StateChanged { state: OperationState },
    StageChanged { stage: String },
    Progress { progress: Progress },
    Completed,
    Failed { error: ErrorSummary },
    Cancelled,
}

impl OperationEventKind {
    fn is_progress(&self) -> bool {
        matches!(self, Self::Progress { .. })
    }

    fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed { .. } | Self::Cancelled
        )
    }
}

#[derive(Debug)]
struct SubscriberState {
    queue: VecDeque<OperationEvent>,
    waiters: Vec<Waker>,
    closed: bool,
}

#[derive(Debug)]
pub(crate) struct Subscriber {
    capacity: usize,
    state: Mutex<SubscriberState>,
}

impl Subscriber {
    pub(crate) fn new(capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            capacity,
            state: Mutex::new(SubscriberState {
                queue: VecDeque::with_capacity(capacity),
                waiters: Vec::new(),
                closed: false,
            }),
        })
    }

    pub(crate) fn push(&self, event: OperationEvent) {
        let terminal = event.kind.is_terminal();
        let waiters = {
            let mut state = self.state.lock().expect("subscriber state poisoned");
            if state.queue.len() >= self.capacity {
                if event.kind.is_progress() {
                    if let Some(index) = state
                        .queue
                        .iter()
                        .rposition(|existing| existing.kind.is_progress())
                    {
                        state.queue[index] = event;
                    }
                    return;
                }
                if let Some(index) = state
                    .queue
                    .iter()
                    .position(|existing| existing.kind.is_progress())
                {
                    state.queue.remove(index);
                } else {
                    state.queue.pop_front();
                }
            }
            state.queue.push_back(event);
            if terminal {
                state.closed = true;
            }
            std::mem::take(&mut state.waiters)
        };
        for waiter in waiters {
            waiter.wake();
        }
    }
}

/// Bounded UI-independent operation event subscription.
///
/// High-frequency progress can be coalesced when the queue is full. State is always queryable from
/// the associated operation handle, and a terminal event is admitted even under backpressure.
#[derive(Debug, Clone)]
pub struct OperationEventStream {
    subscriber: Arc<Subscriber>,
}

impl OperationEventStream {
    pub(crate) fn new(subscriber: Arc<Subscriber>) -> Self {
        Self { subscriber }
    }

    /// Removes the next queued event without blocking.
    #[must_use]
    pub fn try_next(&self) -> Option<OperationEvent> {
        self.subscriber
            .state
            .lock()
            .expect("subscriber state poisoned")
            .queue
            .pop_front()
    }

    /// Asynchronously waits for the next event. Returns `None` after a terminal event has been
    /// drained.
    pub fn next(&self) -> EventNextFuture {
        EventNextFuture {
            subscriber: Arc::clone(&self.subscriber),
        }
    }
}

/// Future returned by [`OperationEventStream::next`].
pub struct EventNextFuture {
    subscriber: Arc<Subscriber>,
}

impl Future for EventNextFuture {
    type Output = Option<OperationEvent>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self
            .subscriber
            .state
            .lock()
            .expect("subscriber state poisoned");
        if let Some(event) = state.queue.pop_front() {
            return Poll::Ready(Some(event));
        }
        if state.closed {
            return Poll::Ready(None);
        }
        if !state
            .waiters
            .iter()
            .any(|waker| waker.will_wake(cx.waker()))
        {
            state.waiters.push(cx.waker().clone());
        }
        Poll::Pending
    }
}

pub(crate) fn terminal_event_kind(result: &OperationResult) -> OperationEventKind {
    match result {
        OperationResult::Succeeded => OperationEventKind::Completed,
        OperationResult::Failed { error } => OperationEventKind::Failed {
            error: error.clone(),
        },
        OperationResult::Cancelled => OperationEventKind::Cancelled,
    }
}
