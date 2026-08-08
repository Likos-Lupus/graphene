use graphene_core::{ErrorCode, ErrorKind, GrapheneError};

pub(crate) fn launch_error(code: ErrorCode, message: &'static str) -> GrapheneError {
    GrapheneError::new(code, ErrorKind::Launch, message)
}
