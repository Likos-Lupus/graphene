use graphene_core::{ErrorCode, ErrorKind, GrapheneError};

pub(crate) fn java_error(code: ErrorCode, message: &'static str) -> GrapheneError {
    let kind = if code == ErrorCode::JavaProbeTimeout {
        ErrorKind::Timeout
    } else {
        ErrorKind::Java
    };
    GrapheneError::new(code, kind, message)
}
