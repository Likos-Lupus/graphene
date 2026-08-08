use graphene_core::{ErrorCode, ErrorKind, GrapheneError};

pub(crate) fn instance_error(message: &'static str) -> GrapheneError {
    GrapheneError::new(ErrorCode::LaunchInstanceInvalid, ErrorKind::Launch, message)
}
