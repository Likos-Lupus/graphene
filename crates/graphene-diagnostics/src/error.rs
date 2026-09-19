use graphene_core::{ErrorCode, ErrorKind, GrapheneError};

/// Builds a structured diagnostics error with the shared diagnostics kind.
pub(crate) fn diagnostic_error(code: ErrorCode, message: impl Into<String>) -> GrapheneError {
    GrapheneError::new(code, ErrorKind::Diagnostics, message)
}

pub(crate) fn request_invalid(message: impl Into<String>) -> GrapheneError {
    diagnostic_error(ErrorCode::DiagnosticRequestInvalid, message)
}

pub(crate) fn report_invalid(message: impl Into<String>) -> GrapheneError {
    diagnostic_error(ErrorCode::DiagnosticReportInvalid, message)
}
