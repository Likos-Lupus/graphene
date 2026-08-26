use graphene_core::{ErrorCode, ErrorKind, GrapheneError};

/// Pack-domain error carrying a stable core [`ErrorCode`] and safe bounded message.
#[derive(Debug)]
pub struct PackError {
    code: ErrorCode,
    message: String,
}

impl PackError {
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::PackSourceInvalid, message)
    }

    #[must_use]
    pub fn archive(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::PackArchiveInvalid, message)
    }

    #[must_use]
    pub fn path(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::PackPathInvalid, message)
    }

    #[must_use]
    pub fn manifest(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::PackManifestInvalid, message)
    }

    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PackError {}

impl From<PackError> for GrapheneError {
    fn from(error: PackError) -> Self {
        GrapheneError::new(error.code, ErrorKind::Modpack, error.message)
    }
}
