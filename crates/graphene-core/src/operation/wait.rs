use super::{OperationRecord, OperationResult};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

/// Runtime-independent future for a retained terminal result.
pub struct OperationWaitFuture {
    pub(crate) record: Arc<OperationRecord>,
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
