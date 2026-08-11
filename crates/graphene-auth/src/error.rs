use graphene_core::{ErrorCode, ErrorKind, GrapheneError};

pub(crate) fn auth_error(code: ErrorCode, message: impl Into<String>) -> GrapheneError {
    GrapheneError::new(code, ErrorKind::Authentication, message)
}
