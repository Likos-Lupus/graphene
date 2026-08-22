use graphene_core::{ErrorCode, ErrorKind, GrapheneError};

pub(crate) fn instance_error(message: impl Into<String>) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::InstanceConfigInvalid,
        ErrorKind::Instance,
        message.into(),
    )
}
