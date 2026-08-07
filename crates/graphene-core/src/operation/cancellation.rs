use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicU8, Ordering}, Arc, Mutex,
        Weak,
    },
    task::{Context, Poll, Waker},
};

const ACTIVE: u8 = 0;
const CANCELLED: u8 = 1;
const SEALED: u8 = 2;

#[derive(Debug)]
struct CancellationInner {
    state: AtomicU8,
    parent: Option<Weak<CancellationInner>>,
    children: Mutex<Vec<Weak<CancellationInner>>>,
    waiters: Mutex<Vec<Waker>>,
    order: Arc<Mutex<()>>,
}

fn cancel_tree_locked(inner: &Arc<CancellationInner>, cancelled: &mut Vec<Arc<CancellationInner>>) {
    if inner
        .state
        .compare_exchange(ACTIVE, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        cancelled.push(Arc::clone(inner));
    }

    let children = {
        let mut children = inner.children.lock().expect("children lock poisoned");
        children.retain(|child| child.strong_count() > 0);
        children
            .iter()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>()
    };
    for child in children {
        cancel_tree_locked(&child, cancelled);
    }
}

fn wake_cancelled(cancelled: Vec<Arc<CancellationInner>>) {
    for inner in cancelled {
        let waiters = std::mem::take(&mut *inner.waiters.lock().expect("waiter lock poisoned"));
        for waiter in waiters {
            waiter.wake();
        }
    }
}

fn ancestor_cancelled(inner: &Arc<CancellationInner>) -> bool {
    let mut parent = inner.parent.as_ref().and_then(Weak::upgrade);
    while let Some(ancestor) = parent {
        if ancestor.state.load(Ordering::Acquire) == CANCELLED {
            return true;
        }
        parent = ancestor.parent.as_ref().and_then(Weak::upgrade);
    }
    false
}

/// Cheap, idempotent cooperative cancellation token with parent-to-child propagation.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    inner: Arc<CancellationInner>,
}

impl CancellationToken {
    /// Creates an independent cancellation root.
    #[must_use]
    pub fn new() -> Self {
        let order = Arc::new(Mutex::new(()));
        Self {
            inner: Arc::new(CancellationInner {
                state: AtomicU8::new(ACTIVE),
                parent: None,
                children: Mutex::new(Vec::new()),
                waiters: Mutex::new(Vec::new()),
                order,
            }),
        }
    }

    /// Creates a child token. Parent cancellation propagates downward; child cancellation does not
    /// cancel the parent.
    #[must_use]
    pub fn child_token(&self) -> Self {
        let child = Self {
            inner: Arc::new(CancellationInner {
                state: AtomicU8::new(ACTIVE),
                parent: Some(Arc::downgrade(&self.inner)),
                children: Mutex::new(Vec::new()),
                waiters: Mutex::new(Vec::new()),
                order: Arc::clone(&self.inner.order),
            }),
        };

        let cancelled = {
            let _order = self
                .inner
                .order
                .lock()
                .expect("cancellation order lock poisoned");
            let mut children = self.inner.children.lock().expect("children lock poisoned");
            children.retain(|existing| existing.strong_count() > 0);
            children.push(Arc::downgrade(&child.inner));
            drop(children);

            let mut cancelled = Vec::new();
            if ancestor_cancelled(&child.inner) {
                cancel_tree_locked(&child.inner, &mut cancelled);
            }
            cancelled
        };
        wake_cancelled(cancelled);
        child
    }

    /// Requests cancellation. Calling this more than once has no additional effect. A sealed token
    /// has crossed an explicitly non-interruptible safe-commit boundary and ignores late requests.
    pub fn cancel(&self) {
        let cancelled = {
            let _order = self
                .inner
                .order
                .lock()
                .expect("cancellation order lock poisoned");
            let mut cancelled = Vec::new();
            cancel_tree_locked(&self.inner, &mut cancelled);
            cancelled
        };
        wake_cancelled(cancelled);
    }

    /// Returns whether cancellation has been requested before the non-interruptible boundary.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.inner.state.load(Ordering::Acquire) == CANCELLED
    }

    /// Atomically seals cancellation at a documented point of no return. Returns `false` when
    /// cancellation already won the race. This is intended only for tiny non-interruptible safe-commit
    /// sections.
    #[must_use]
    pub fn try_seal(&self) -> bool {
        let (sealed, cancelled) = {
            let _order = self
                .inner
                .order
                .lock()
                .expect("cancellation order lock poisoned");
            if ancestor_cancelled(&self.inner) {
                let mut cancelled = Vec::new();
                cancel_tree_locked(&self.inner, &mut cancelled);
                (false, cancelled)
            } else {
                let sealed = match self.inner.state.compare_exchange(
                    ACTIVE,
                    SEALED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) | Err(SEALED) => true,
                    Err(CANCELLED) => false,
                    Err(_) => false,
                };
                (sealed, Vec::new())
            }
        };
        wake_cancelled(cancelled);
        sealed
    }

    /// Resolves when cancellation is requested without depending on a particular async runtime.
    pub fn cancelled(&self) -> CancellationFuture {
        CancellationFuture {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Future returned by [`CancellationToken::cancelled`].
pub struct CancellationFuture {
    inner: Arc<CancellationInner>,
}

impl Future for CancellationFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.inner.state.load(Ordering::Acquire) == CANCELLED {
            return Poll::Ready(());
        }
        let mut waiters = self.inner.waiters.lock().expect("waiter lock poisoned");
        if self.inner.state.load(Ordering::Acquire) == CANCELLED {
            return Poll::Ready(());
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

    #[test]
    fn cancellation_is_idempotent_and_propagates_to_children() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        parent.cancel();
        parent.cancel();
        assert!(parent.is_cancelled());
        assert!(child.is_cancelled());
    }

    #[test]
    fn child_cancellation_does_not_cancel_parent() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        child.cancel();
        assert!(child.is_cancelled());
        assert!(!parent.is_cancelled());
    }

    #[test]
    fn parent_cancellation_before_seal_wins() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        parent.cancel();
        assert!(!child.try_seal());
        assert!(child.is_cancelled());
    }

    #[test]
    fn sealed_child_ignores_late_parent_cancellation() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        assert!(child.try_seal());
        parent.cancel();
        assert!(parent.is_cancelled());
        assert!(!child.is_cancelled());
    }

    #[test]
    fn sealed_child_does_not_shield_active_descendant_from_parent_cancellation() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        let grandchild = child.child_token();
        assert!(child.try_seal());

        parent.cancel();

        assert!(parent.is_cancelled());
        assert!(!child.is_cancelled());
        assert!(grandchild.is_cancelled());
    }
}
