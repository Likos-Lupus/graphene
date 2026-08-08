use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, error::Error, fmt};

/// Stable machine-readable Graphene error code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum ErrorCode {
    ConfigInvalid,
    DataRootInvalid,
    DirectoryCreateFailed,
    FileOpenFailed,
    FileWriteFailed,
    FileRenameFailed,
    NetworkRequestFailed,
    NetworkTimeout,
    NetworkStatusError,
    NetworkRedirectRejected,
    NetworkProxyInvalid,
    DownloadCancelled,
    DownloadSizeMismatch,
    HashMismatch,
    CacheIdentityUnavailable,
    CacheCommitFailed,
    StorageLayoutInvalid,
    OperationCancelled,
    OperationStateInvalid,
    PlatformUnsupported,
    MinecraftManifestInvalid,
    MinecraftVersionNotFound,
    MinecraftMetadataInvalid,
    MinecraftMetadataUnsupported,
    MinecraftInheritanceCycle,
    MinecraftInheritanceTooDeep,
    MinecraftRuleInvalid,
    MinecraftLibraryInvalid,
    MinecraftAssetIndexInvalid,
    MinecraftNativeUnavailable,
    MinecraftArgumentInvalid,
    InstallRequestInvalid,
    InstallPlanInvalid,
    InstallTargetExists,
    InstallStageFailed,
    InstallNativeExtractionFailed,
    InstallValidationFailed,
    InstallCommitFailed,
    InstallCancelled,
    JavaNotFound,
    JavaProbeFailed,
    JavaProbeTimeout,
    JavaIncompatible,
    LaunchInstanceInvalid,
    LaunchSessionInvalid,
    LaunchPlaceholderMissing,
    LaunchPlanInvalid,
    LaunchProcessSpawnFailed,
    LaunchProcessIoFailed,
    InternalInvariantViolation,
}

impl ErrorCode {
    /// Returns the stable external string representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConfigInvalid => "CONFIG_INVALID",
            Self::DataRootInvalid => "DATA_ROOT_INVALID",
            Self::DirectoryCreateFailed => "DIRECTORY_CREATE_FAILED",
            Self::FileOpenFailed => "FILE_OPEN_FAILED",
            Self::FileWriteFailed => "FILE_WRITE_FAILED",
            Self::FileRenameFailed => "FILE_RENAME_FAILED",
            Self::NetworkRequestFailed => "NETWORK_REQUEST_FAILED",
            Self::NetworkTimeout => "NETWORK_TIMEOUT",
            Self::NetworkStatusError => "NETWORK_STATUS_ERROR",
            Self::NetworkRedirectRejected => "NETWORK_REDIRECT_REJECTED",
            Self::NetworkProxyInvalid => "NETWORK_PROXY_INVALID",
            Self::DownloadCancelled => "DOWNLOAD_CANCELLED",
            Self::DownloadSizeMismatch => "DOWNLOAD_SIZE_MISMATCH",
            Self::HashMismatch => "HASH_MISMATCH",
            Self::CacheIdentityUnavailable => "CACHE_IDENTITY_UNAVAILABLE",
            Self::CacheCommitFailed => "CACHE_COMMIT_FAILED",
            Self::StorageLayoutInvalid => "STORAGE_LAYOUT_INVALID",
            Self::OperationCancelled => "OPERATION_CANCELLED",
            Self::OperationStateInvalid => "OPERATION_STATE_INVALID",
            Self::PlatformUnsupported => "PLATFORM_UNSUPPORTED",
            Self::MinecraftManifestInvalid => "MINECRAFT_MANIFEST_INVALID",
            Self::MinecraftVersionNotFound => "MINECRAFT_VERSION_NOT_FOUND",
            Self::MinecraftMetadataInvalid => "MINECRAFT_METADATA_INVALID",
            Self::MinecraftMetadataUnsupported => "MINECRAFT_METADATA_UNSUPPORTED",
            Self::MinecraftInheritanceCycle => "MINECRAFT_INHERITANCE_CYCLE",
            Self::MinecraftInheritanceTooDeep => "MINECRAFT_INHERITANCE_TOO_DEEP",
            Self::MinecraftRuleInvalid => "MINECRAFT_RULE_INVALID",
            Self::MinecraftLibraryInvalid => "MINECRAFT_LIBRARY_INVALID",
            Self::MinecraftAssetIndexInvalid => "MINECRAFT_ASSET_INDEX_INVALID",
            Self::MinecraftNativeUnavailable => "MINECRAFT_NATIVE_UNAVAILABLE",
            Self::MinecraftArgumentInvalid => "MINECRAFT_ARGUMENT_INVALID",
            Self::InstallRequestInvalid => "INSTALL_REQUEST_INVALID",
            Self::InstallPlanInvalid => "INSTALL_PLAN_INVALID",
            Self::InstallTargetExists => "INSTALL_TARGET_EXISTS",
            Self::InstallStageFailed => "INSTALL_STAGE_FAILED",
            Self::InstallNativeExtractionFailed => "INSTALL_NATIVE_EXTRACTION_FAILED",
            Self::InstallValidationFailed => "INSTALL_VALIDATION_FAILED",
            Self::InstallCommitFailed => "INSTALL_COMMIT_FAILED",
            Self::InstallCancelled => "INSTALL_CANCELLED",
            Self::JavaNotFound => "JAVA_NOT_FOUND",
            Self::JavaProbeFailed => "JAVA_PROBE_FAILED",
            Self::JavaProbeTimeout => "JAVA_PROBE_TIMEOUT",
            Self::JavaIncompatible => "JAVA_INCOMPATIBLE",
            Self::LaunchInstanceInvalid => "LAUNCH_INSTANCE_INVALID",
            Self::LaunchSessionInvalid => "LAUNCH_SESSION_INVALID",
            Self::LaunchPlaceholderMissing => "LAUNCH_PLACEHOLDER_MISSING",
            Self::LaunchPlanInvalid => "LAUNCH_PLAN_INVALID",
            Self::LaunchProcessSpawnFailed => "LAUNCH_PROCESS_SPAWN_FAILED",
            Self::LaunchProcessIoFailed => "LAUNCH_PROCESS_IO_FAILED",
            Self::InternalInvariantViolation => "INTERNAL_INVARIANT_VIOLATION",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Broad machine-readable error category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ErrorKind {
    Configuration,
    Platform,
    Filesystem,
    Network,
    Timeout,
    Cancelled,
    Integrity,
    Storage,
    Minecraft,
    Install,
    Java,
    Launch,
    Internal,
}

/// Structured non-secret diagnostic context attached to an error.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorContext(BTreeMap<String, String>);

impl ErrorContext {
    /// Creates empty context.
    #[must_use]
    pub const fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Adds a non-sensitive key/value pair.
    #[must_use]
    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.0.insert(key.into(), value.into());
        self
    }

    /// Reads one context value.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    /// Iterates context fields in deterministic key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }
}

/// Cloneable public summary suitable for events and diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorSummary {
    pub code: ErrorCode,
    pub kind: ErrorKind,
    pub context: ErrorContext,
    pub message: String,
}

/// Structured Graphene-owned error contract.
pub struct GrapheneError {
    pub code: ErrorCode,
    pub kind: ErrorKind,
    pub context: ErrorContext,
    message: String,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl GrapheneError {
    /// Creates a structured error without a source chain.
    #[must_use]
    pub fn new(code: ErrorCode, kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            code,
            kind,
            context: ErrorContext::new(),
            message: message.into(),
            source: None,
        }
    }

    /// Adds a non-sensitive context field.
    #[must_use]
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context = self.context.with(key, value);
        self
    }

    /// Attaches an implementation error as the source chain.
    #[must_use]
    pub fn with_source<E>(mut self, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        self.source = Some(Box::new(source));
        self
    }

    /// Returns a cloneable error summary without the concrete source type.
    #[must_use]
    pub fn summary(&self) -> ErrorSummary {
        ErrorSummary {
            code: self.code,
            kind: self.kind,
            context: self.context.clone(),
            message: self.message.clone(),
        }
    }

    /// Returns whether this error represents cooperative cancellation.
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        matches!(self.kind, ErrorKind::Cancelled)
    }

    /// Returns the developer-facing message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Debug for GrapheneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GrapheneError")
            .field("code", &self.code)
            .field("kind", &self.kind)
            .field("context", &self.context)
            .field("message", &self.message)
            .field("source", &self.source.as_ref().map(|_| "<source>"))
            .finish()
    }
}

impl fmt::Display for GrapheneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl Error for GrapheneError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable_machine_readable_values() {
        assert_eq!(ErrorCode::ConfigInvalid.as_str(), "CONFIG_INVALID");
        assert_eq!(ErrorCode::HashMismatch.as_str(), "HASH_MISMATCH");
        assert_eq!(
            ErrorCode::OperationCancelled.as_str(),
            "OPERATION_CANCELLED"
        );
    }

    #[test]
    fn structured_error_keeps_fields_separate() {
        let error = GrapheneError::new(
            ErrorCode::DownloadSizeMismatch,
            ErrorKind::Integrity,
            "size mismatch",
        )
        .with_context("expected", "10")
        .with_context("actual", "9");
        assert_eq!(error.code, ErrorCode::DownloadSizeMismatch);
        assert_eq!(error.context.get("expected"), Some("10"));
    }
}
