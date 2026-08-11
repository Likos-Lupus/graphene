use graphene_core::{ErrorCode, ErrorKind, GrapheneError};

pub(super) const MAX_AUTH_RESPONSE_BYTES: usize = 256 * 1024;
pub(super) const MAX_PROVIDER_STRING_BYTES: usize = 4096;

pub(super) fn provider_error(
    code: ErrorCode,
    stage: &'static str,
    status: Option<u16>,
    message: &'static str,
) -> GrapheneError {
    let mut error =
        GrapheneError::new(code, ErrorKind::Authentication, message).with_context("stage", stage);
    if let Some(status) = status {
        error = error.with_context("status", status.to_string());
    }
    error
}

pub(super) fn validate_string(value: &str, stage: &'static str) -> Result<(), GrapheneError> {
    if value.is_empty() || value.len() > MAX_PROVIDER_STRING_BYTES {
        Err(provider_error(
            ErrorCode::AuthProviderUnavailable,
            stage,
            None,
            "authentication provider returned an invalid bounded field",
        ))
    } else {
        Ok(())
    }
}
