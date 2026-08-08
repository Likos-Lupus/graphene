use graphene_core::{ErrorCode, ErrorKind, GrapheneError};

pub(crate) fn cancelled_error() -> GrapheneError {
    GrapheneError::new(
        ErrorCode::InstallCancelled,
        ErrorKind::Cancelled,
        "installation was cancelled",
    )
}

pub(crate) fn install_error(code: ErrorCode, message: &'static str) -> GrapheneError {
    GrapheneError::new(code, ErrorKind::Install, message)
}
