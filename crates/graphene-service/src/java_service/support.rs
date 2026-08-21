use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};

pub(super) async fn spawn_blocking_java<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work).await.map_err(|source| {
        java_error(
            ErrorCode::JavaManagedInstallFailed,
            "blocking managed Java task failed",
        )
        .with_source(source)
    })?
}

pub(super) fn java_error(code: ErrorCode, message: &'static str) -> GrapheneError {
    GrapheneError::new(code, ErrorKind::Java, message)
}

pub(super) fn cancelled() -> GrapheneError {
    GrapheneError::new(
        ErrorCode::OperationCancelled,
        ErrorKind::Cancelled,
        "managed Java operation was cancelled",
    )
}
