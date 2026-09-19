use graphene_core::{OperationHandle, Result};
use graphene_diagnostics::DiagnosticReport;
use std::{future::Future, pin::Pin};

/// Prepared asynchronous diagnostic analysis operation.
pub struct DiagnosticAnalysisOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = Result<DiagnosticReport>> + Send>>,
}

impl DiagnosticAnalysisOperation {
    pub(crate) fn new(
        operation: OperationHandle,
        future: Pin<Box<dyn Future<Output = Result<DiagnosticReport>> + Send>>,
    ) -> Self {
        Self { operation, future }
    }

    /// Returns the public operation handle for progress observation and cancellation.
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }

    /// Awaits completion of the bounded diagnostic analysis.
    pub async fn await_result(self) -> Result<DiagnosticReport> {
        self.future.await
    }
}

impl std::fmt::Debug for DiagnosticAnalysisOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiagnosticAnalysisOperation")
            .field("operation_id", &self.operation.id())
            .finish_non_exhaustive()
    }
}
