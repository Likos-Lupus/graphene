use graphene_core::{AccountId, ErrorCode, ErrorKind, GrapheneError};

pub(super) fn auth_error(code: ErrorCode, message: &'static str) -> GrapheneError {
    GrapheneError::new(code, ErrorKind::Authentication, message)
}

pub(super) fn account_not_found(id: AccountId) -> GrapheneError {
    auth_error(ErrorCode::AuthAccountNotFound, "account was not found")
        .with_context("account_id", id.to_string())
}

pub(super) fn cancelled() -> GrapheneError {
    GrapheneError::new(
        ErrorCode::AuthCancelled,
        ErrorKind::Cancelled,
        "authentication operation was cancelled",
    )
}
