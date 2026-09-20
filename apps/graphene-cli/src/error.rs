use graphene::{ErrorCode, ErrorKind, GrapheneError};
use serde::Serialize;
use std::collections::BTreeMap;

/// Stable, safe machine-readable error envelope shared by CLI output and tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub kind: String,
    pub message: String,
    pub context: BTreeMap<String, String>,
}

impl ErrorBody {
    #[must_use]
    pub fn from_error(error: &GrapheneError) -> Self {
        Self {
            code: error.code.as_str().to_owned(),
            kind: format!("{:?}", error.kind),
            message: error.message().to_owned(),
            context: error
                .context
                .iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect(),
        }
    }
}

/// Coarse, documented exit-code mapping. Detailed error identity stays in the JSON envelope.
#[must_use]
pub const fn exit_code(error: &GrapheneError) -> i32 {
    if matches!(error.kind, ErrorKind::Cancelled) {
        130
    } else {
        1
    }
}

/// Standard host cancellation error used when an interrupt requests cooperative cancellation.
#[must_use]
pub fn cancelled() -> GrapheneError {
    GrapheneError::new(
        ErrorCode::OperationCancelled,
        ErrorKind::Cancelled,
        "operation cancelled by user",
    )
}
