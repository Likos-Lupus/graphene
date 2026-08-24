use graphene_core::{ErrorCode, ErrorKind, GrapheneError};

/// Helper constructors for content-domain errors.
pub fn incompatible(message: impl Into<String>) -> GrapheneError {
    GrapheneError::new(ErrorCode::ContentIncompatible, ErrorKind::Content, message)
}

pub fn project_not_found(message: impl Into<String>) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::ContentProjectNotFound,
        ErrorKind::Content,
        message,
    )
}

pub fn version_not_found(message: impl Into<String>) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::ContentVersionNotFound,
        ErrorKind::Content,
        message,
    )
}

pub fn provider_unavailable(message: impl Into<String>) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::ContentProviderUnavailable,
        ErrorKind::Content,
        message,
    )
}

pub fn dependency_conflict(message: impl Into<String>) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::ContentDependencyConflict,
        ErrorKind::Content,
        message,
    )
}

pub fn dependency_unsatisfied(message: impl Into<String>) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::ContentDependencyUnsatisfied,
        ErrorKind::Content,
        message,
    )
}

pub fn plan_invalid(message: impl Into<String>) -> GrapheneError {
    GrapheneError::new(ErrorCode::ContentPlanInvalid, ErrorKind::Content, message)
}

pub fn inventory_stale(message: impl Into<String>) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::ContentInventoryStale,
        ErrorKind::Content,
        message,
    )
}
