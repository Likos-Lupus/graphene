use crate::context::ServiceContext;
use graphene_core::{
    ErrorCode, ErrorKind, GrapheneError, OperationController, OperationHandle, OperationResult,
    Progress,
};
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
use tracing::debug;

/// Unified operation runtime access for one engine.
#[derive(Clone)]
pub struct OperationService {
    context: Arc<ServiceContext>,
}

impl std::fmt::Debug for OperationService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OperationService").finish_non_exhaustive()
    }
}

impl OperationService {
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        Self { context }
    }

    /// Creates a root operation controller. This is useful for grouping child work under a host-
    /// meaningful parent operation.
    #[must_use]
    pub fn create(&self, name: impl Into<String>) -> OperationController {
        let controller = self.context.operations.create(name);
        debug!(
            operation_id = %controller.handle().id(),
            "root operation created"
        );
        controller
    }

    /// Creates a child operation controller.
    #[must_use]
    pub fn create_child(
        &self,
        parent: &OperationHandle,
        name: impl Into<String>,
    ) -> OperationController {
        let controller = self.context.operations.create_child(parent, name);
        debug!(
            operation_id = %controller.handle().id(),
            parent_id = %parent.id(),
            "child operation created"
        );
        controller
    }

    /// Creates a deterministic synthetic long-running operation that demonstrates observation,
    /// parent-child linkage, item progress, cancellation, and terminal semantics without any
    /// product-specific functionality.
    #[must_use]
    pub fn synthetic(
        &self,
        parent: Option<&OperationHandle>,
        steps: u64,
        step_delay: Duration,
    ) -> SyntheticOperation {
        let controller = parent.map_or_else(
            || self.create("synthetic"),
            |parent| self.create_child(parent, "synthetic"),
        );
        let operation = controller.handle();
        let future = Box::pin(async move {
            if let Err(error) = controller.start() {
                let _ = controller.cancelled();
                return Err(error);
            }

            if steps == 0 {
                let error = GrapheneError::new(
                    ErrorCode::ConfigInvalid,
                    ErrorKind::Configuration,
                    "synthetic operation requires at least one step",
                );
                let _ = controller.fail(error.summary());
                return Err(error);
            }

            controller.set_stage("synthetic-work")?;
            controller.set_progress(Progress::Items {
                completed: 0,
                total: Some(steps),
            })?;
            for completed in 1..=steps {
                let token = controller.cancellation_token();
                tokio::select! {
                    () = token.cancelled() => {
                        let _ = controller.cancelled();
                        return Err(cancelled_error());
                    }

                    () = tokio::time::sleep(step_delay) => {}
                }

                if controller.is_cancelled() {
                    let _ = controller.cancelled();
                    return Err(cancelled_error());
                }

                controller.set_progress(Progress::Items {
                    completed,
                    total: Some(steps),
                })?;
            }

            match controller.succeed()? {
                OperationResult::Succeeded => {
                    debug!(
                        operation_id = %controller.handle().id(),
                        state = "Succeeded",
                        "synthetic operation reached terminal state"
                    );
                    Ok(())
                }
                OperationResult::Cancelled => {
                    debug!(
                        operation_id = %controller.handle().id(),
                        state = "Cancelled",
                        "synthetic operation reached terminal state"
                    );
                    Err(cancelled_error())
                }
                OperationResult::Failed { .. } => Err(GrapheneError::new(
                    ErrorCode::InternalInvariantViolation,
                    ErrorKind::Internal,
                    "synthetic operation unexpectedly failed during success transition",
                )),
            }
        });

        SyntheticOperation { operation, future }
    }
}

/// Opaque future + operation handle for the synthetic Phase 0 demonstration.
pub struct SyntheticOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = graphene_core::Result<()>> + Send + 'static>>,
}

impl std::fmt::Debug for SyntheticOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyntheticOperation")
            .field("operation", &self.operation)
            .finish_non_exhaustive()
    }
}

impl SyntheticOperation {
    /// Returns a cloneable observer/cancellation handle before work is awaited.
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }

    /// Runs and awaits the synthetic operation.
    pub async fn await_result(self) -> graphene_core::Result<()> {
        self.future.await
    }
}

fn cancelled_error() -> GrapheneError {
    GrapheneError::new(
        ErrorCode::OperationCancelled,
        ErrorKind::Cancelled,
        "synthetic operation was cancelled",
    )
}
