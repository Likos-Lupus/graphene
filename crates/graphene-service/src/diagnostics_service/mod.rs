//! Bounded, read-only diagnostic orchestration over existing Graphene services.

mod collection;
mod operation;
mod orchestration;
#[cfg(test)]
mod tests;

pub use operation::DiagnosticAnalysisOperation;

use crate::context::ServiceContext;
use graphene_core::InstanceId;
use graphene_diagnostics::DiagnosticRequest;
use std::sync::Arc;

/// Read-only diagnostic service that collects bounded local evidence and returns a structured,
/// secret-redacted report. It never mutates instance, content, Java, or account state.
#[derive(Clone)]
pub struct DiagnosticService {
    context: Arc<ServiceContext>,
}

impl std::fmt::Debug for DiagnosticService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiagnosticService")
            .field("data_root", &self.context.storage.path())
            .finish_non_exhaustive()
    }
}

impl DiagnosticService {
    #[must_use]
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        Self { context }
    }

    /// Starts a bounded, read-only diagnostic analysis operation for one instance.
    #[must_use]
    pub fn analyze(
        &self,
        instance_id: InstanceId,
        request: DiagnosticRequest,
    ) -> DiagnosticAnalysisOperation {
        orchestration::start_analysis(Arc::clone(&self.context), instance_id, request)
    }
}
