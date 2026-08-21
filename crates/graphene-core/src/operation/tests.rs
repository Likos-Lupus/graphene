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
