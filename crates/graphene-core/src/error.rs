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
    ComponentInvalid,
    ComponentGraphInvalid,
    ComponentCycle,
    ComponentRequirementUnsatisfied,
    ComponentConflict,
    ComponentPatchInvalid,
    ComponentJavaConflict,
    LoaderProviderUnavailable,
    LoaderVersionNotFound,
    LoaderVersionUnsupported,
    LoaderMetadataInvalid,
    LoaderProfileInvalid,
    LoaderInstallerInvalid,
    LoaderInstallerSpecUnsupported,
    LoaderArtifactUnverifiable,
    LoaderProcessorUnsupported,
    LoaderProcessorPlaceholderInvalid,
    LoaderProcessorJavaUnavailable,
    LoaderProcessorFailed,
    LoaderProcessorTimeout,
    LoaderProcessorOutputMissing,
    LoaderProcessorOutputMismatch,
    LoaderProcessorCancelled,
    InstallRequestInvalid,
    InstallPlanInvalid,
    InstallTargetExists,
    InstallStageFailed,
    InstallNativeExtractionFailed,
    InstallValidationFailed,
    InstallCommitFailed,
    InstallCancelled,
    InstanceNotFound,
    InstanceInvalid,
    InstanceBusy,
    InstanceConfigInvalid,
    InstanceStateInvalid,
    InstanceLockfileInvalid,
    InstanceMutationFailed,
    InstanceCloneFailed,
    InstanceDeleteFailed,
    InstanceRepairPlanStale,
    InstanceRepairFailed,
    JavaNotFound,
    JavaProbeFailed,
    JavaProbeTimeout,
    JavaIncompatible,
    JavaVersionTooOld,
    JavaVersionTooNew,
    JavaArchitectureMismatch,
    JavaRuntimeUnexecutable,
    JavaManagedProviderUnavailable,
    JavaManagedReleaseUnavailable,
    JavaManagedArchiveInvalid,
    JavaManagedRuntimeCorrupt,
    JavaManagedInstallFailed,
    LaunchInstanceInvalid,
    LaunchSessionInvalid,
    LaunchPlaceholderMissing,
    LaunchPlanInvalid,
    LaunchProcessSpawnFailed,
    LaunchProcessIoFailed,
    AuthRequestInvalid,
    AuthProviderUnavailable,
    AuthInteractionExpired,
    AuthInteractionRequired,
    AuthDeviceAuthorizationRejected,
    AuthRefreshRejected,
    AuthXboxRejected,
    AuthXstsRejected,
    AuthMinecraftRejected,
    AuthEntitlementMissing,
    AuthProfileMissing,
    AuthAccountNotFound,
    AuthAccountStateInvalid,
    AuthSecretStoreUnavailable,
    AuthSecretReadFailed,
    AuthSecretWriteFailed,
    AuthSecretDeleteFailed,
    AuthPersistenceFailed,
    AuthCancelled,
    ContentProviderUnavailable,
    ContentProjectNotFound,
    ContentVersionNotFound,
    ContentFileUnavailable,
    ContentFileUnverifiable,
    ContentIncompatible,
    ContentDependencyUnsatisfied,
    ContentDependencyConflict,
    ContentDependencyBoundExceeded,
    ContentAmbiguousMatch,
    ContentInventoryStale,
    ContentPlanInvalid,
    ContentMutationFailed,
    ContentRecoveryFailed,
    ArtifactSourceUnavailable,
    PackSourceInvalid,
    PackSourceTooLarge,
    PackSourceSnapshotUnavailable,
    PackArchiveInvalid,
    PackArchiveUnsupported,
    PackPathInvalid,
    PackFormatUnknown,
    PackFormatAmbiguous,
    PackManifestInvalid,
    PackRuntimeUnsupported,
    PackSelectionRequired,
    PackArtifactUnverifiable,
    PackArtifactSourceUnsafe,
    PackProviderResolutionFailed,
    PackPlanInvalid,
    PackPlanStale,
    PackImportFailed,
    PackExportInvalid,
    PackExportStale,
    PackEmbeddingDecisionRequired,
    PackOutputExists,
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
            Self::ComponentInvalid => "COMPONENT_INVALID",
            Self::ComponentGraphInvalid => "COMPONENT_GRAPH_INVALID",
            Self::ComponentCycle => "COMPONENT_CYCLE",
            Self::ComponentRequirementUnsatisfied => "COMPONENT_REQUIREMENT_UNSATISFIED",
            Self::ComponentConflict => "COMPONENT_CONFLICT",
            Self::ComponentPatchInvalid => "COMPONENT_PATCH_INVALID",
            Self::ComponentJavaConflict => "COMPONENT_JAVA_CONFLICT",
            Self::LoaderProviderUnavailable => "LOADER_PROVIDER_UNAVAILABLE",
            Self::LoaderVersionNotFound => "LOADER_VERSION_NOT_FOUND",
            Self::LoaderVersionUnsupported => "LOADER_VERSION_UNSUPPORTED",
            Self::LoaderMetadataInvalid => "LOADER_METADATA_INVALID",
            Self::LoaderProfileInvalid => "LOADER_PROFILE_INVALID",
            Self::LoaderInstallerInvalid => "LOADER_INSTALLER_INVALID",
            Self::LoaderInstallerSpecUnsupported => "LOADER_INSTALLER_SPEC_UNSUPPORTED",
            Self::LoaderArtifactUnverifiable => "LOADER_ARTIFACT_UNVERIFIABLE",
            Self::LoaderProcessorUnsupported => "LOADER_PROCESSOR_UNSUPPORTED",
            Self::LoaderProcessorPlaceholderInvalid => "LOADER_PROCESSOR_PLACEHOLDER_INVALID",
            Self::LoaderProcessorJavaUnavailable => "LOADER_PROCESSOR_JAVA_UNAVAILABLE",
            Self::LoaderProcessorFailed => "LOADER_PROCESSOR_FAILED",
            Self::LoaderProcessorTimeout => "LOADER_PROCESSOR_TIMEOUT",
            Self::LoaderProcessorOutputMissing => "LOADER_PROCESSOR_OUTPUT_MISSING",
            Self::LoaderProcessorOutputMismatch => "LOADER_PROCESSOR_OUTPUT_MISMATCH",
            Self::LoaderProcessorCancelled => "LOADER_PROCESSOR_CANCELLED",
            Self::InstallRequestInvalid => "INSTALL_REQUEST_INVALID",
            Self::InstallPlanInvalid => "INSTALL_PLAN_INVALID",
            Self::InstallTargetExists => "INSTALL_TARGET_EXISTS",
            Self::InstallStageFailed => "INSTALL_STAGE_FAILED",
            Self::InstallNativeExtractionFailed => "INSTALL_NATIVE_EXTRACTION_FAILED",
            Self::InstallValidationFailed => "INSTALL_VALIDATION_FAILED",
            Self::InstallCommitFailed => "INSTALL_COMMIT_FAILED",
            Self::InstallCancelled => "INSTALL_CANCELLED",
            Self::InstanceNotFound => "INSTANCE_NOT_FOUND",
            Self::InstanceInvalid => "INSTANCE_INVALID",
            Self::InstanceBusy => "INSTANCE_BUSY",
            Self::InstanceConfigInvalid => "INSTANCE_CONFIG_INVALID",
            Self::InstanceStateInvalid => "INSTANCE_STATE_INVALID",
            Self::InstanceLockfileInvalid => "INSTANCE_LOCKFILE_INVALID",
            Self::InstanceMutationFailed => "INSTANCE_MUTATION_FAILED",
            Self::InstanceCloneFailed => "INSTANCE_CLONE_FAILED",
            Self::InstanceDeleteFailed => "INSTANCE_DELETE_FAILED",
            Self::InstanceRepairPlanStale => "INSTANCE_REPAIR_PLAN_STALE",
            Self::InstanceRepairFailed => "INSTANCE_REPAIR_FAILED",
            Self::JavaNotFound => "JAVA_NOT_FOUND",
            Self::JavaProbeFailed => "JAVA_PROBE_FAILED",
            Self::JavaProbeTimeout => "JAVA_PROBE_TIMEOUT",
            Self::JavaIncompatible => "JAVA_INCOMPATIBLE",
            Self::JavaVersionTooOld => "JAVA_VERSION_TOO_OLD",
            Self::JavaVersionTooNew => "JAVA_VERSION_TOO_NEW",
            Self::JavaArchitectureMismatch => "JAVA_ARCHITECTURE_MISMATCH",
            Self::JavaRuntimeUnexecutable => "JAVA_RUNTIME_UNEXECUTABLE",
            Self::JavaManagedProviderUnavailable => "JAVA_MANAGED_PROVIDER_UNAVAILABLE",
            Self::JavaManagedReleaseUnavailable => "JAVA_MANAGED_RELEASE_UNAVAILABLE",
            Self::JavaManagedArchiveInvalid => "JAVA_MANAGED_ARCHIVE_INVALID",
            Self::JavaManagedRuntimeCorrupt => "JAVA_MANAGED_RUNTIME_CORRUPT",
            Self::JavaManagedInstallFailed => "JAVA_MANAGED_INSTALL_FAILED",
            Self::LaunchInstanceInvalid => "LAUNCH_INSTANCE_INVALID",
            Self::LaunchSessionInvalid => "LAUNCH_SESSION_INVALID",
            Self::LaunchPlaceholderMissing => "LAUNCH_PLACEHOLDER_MISSING",
            Self::LaunchPlanInvalid => "LAUNCH_PLAN_INVALID",
            Self::LaunchProcessSpawnFailed => "LAUNCH_PROCESS_SPAWN_FAILED",
            Self::LaunchProcessIoFailed => "LAUNCH_PROCESS_IO_FAILED",
            Self::AuthRequestInvalid => "AUTH_REQUEST_INVALID",
            Self::AuthProviderUnavailable => "AUTH_PROVIDER_UNAVAILABLE",
            Self::AuthInteractionExpired => "AUTH_INTERACTION_EXPIRED",
            Self::AuthInteractionRequired => "AUTH_INTERACTION_REQUIRED",
            Self::AuthDeviceAuthorizationRejected => "AUTH_DEVICE_AUTHORIZATION_REJECTED",
            Self::AuthRefreshRejected => "AUTH_REFRESH_REJECTED",
            Self::AuthXboxRejected => "AUTH_XBOX_REJECTED",
            Self::AuthXstsRejected => "AUTH_XSTS_REJECTED",
            Self::AuthMinecraftRejected => "AUTH_MINECRAFT_REJECTED",
            Self::AuthEntitlementMissing => "AUTH_ENTITLEMENT_MISSING",
            Self::AuthProfileMissing => "AUTH_PROFILE_MISSING",
            Self::AuthAccountNotFound => "AUTH_ACCOUNT_NOT_FOUND",
            Self::AuthAccountStateInvalid => "AUTH_ACCOUNT_STATE_INVALID",
            Self::AuthSecretStoreUnavailable => "AUTH_SECRET_STORE_UNAVAILABLE",
            Self::AuthSecretReadFailed => "AUTH_SECRET_READ_FAILED",
            Self::AuthSecretWriteFailed => "AUTH_SECRET_WRITE_FAILED",
            Self::AuthSecretDeleteFailed => "AUTH_SECRET_DELETE_FAILED",
            Self::AuthPersistenceFailed => "AUTH_PERSISTENCE_FAILED",
            Self::AuthCancelled => "AUTH_CANCELLED",
            Self::ContentProviderUnavailable => "CONTENT_PROVIDER_UNAVAILABLE",
            Self::ContentProjectNotFound => "CONTENT_PROJECT_NOT_FOUND",
            Self::ContentVersionNotFound => "CONTENT_VERSION_NOT_FOUND",
            Self::ContentFileUnavailable => "CONTENT_FILE_UNAVAILABLE",
            Self::ContentFileUnverifiable => "CONTENT_FILE_UNVERIFIABLE",
            Self::ContentIncompatible => "CONTENT_INCOMPATIBLE",
            Self::ContentDependencyUnsatisfied => "CONTENT_DEPENDENCY_UNSATISFIED",
            Self::ContentDependencyConflict => "CONTENT_DEPENDENCY_CONFLICT",
            Self::ContentDependencyBoundExceeded => "CONTENT_DEPENDENCY_BOUND_EXCEEDED",
            Self::ContentAmbiguousMatch => "CONTENT_AMBIGUOUS_MATCH",
            Self::ContentInventoryStale => "CONTENT_INVENTORY_STALE",
            Self::ContentPlanInvalid => "CONTENT_PLAN_INVALID",
            Self::ContentMutationFailed => "CONTENT_MUTATION_FAILED",
            Self::ContentRecoveryFailed => "CONTENT_RECOVERY_FAILED",
            Self::ArtifactSourceUnavailable => "ARTIFACT_SOURCE_UNAVAILABLE",
            Self::PackSourceInvalid => "PACK_SOURCE_INVALID",
            Self::PackSourceTooLarge => "PACK_SOURCE_TOO_LARGE",
            Self::PackSourceSnapshotUnavailable => "PACK_SOURCE_SNAPSHOT_UNAVAILABLE",
            Self::PackArchiveInvalid => "PACK_ARCHIVE_INVALID",
            Self::PackArchiveUnsupported => "PACK_ARCHIVE_UNSUPPORTED",
            Self::PackPathInvalid => "PACK_PATH_INVALID",
            Self::PackFormatUnknown => "PACK_FORMAT_UNKNOWN",
            Self::PackFormatAmbiguous => "PACK_FORMAT_AMBIGUOUS",
            Self::PackManifestInvalid => "PACK_MANIFEST_INVALID",
            Self::PackRuntimeUnsupported => "PACK_RUNTIME_UNSUPPORTED",
            Self::PackSelectionRequired => "PACK_SELECTION_REQUIRED",
            Self::PackArtifactUnverifiable => "PACK_ARTIFACT_UNVERIFIABLE",
            Self::PackArtifactSourceUnsafe => "PACK_ARTIFACT_SOURCE_UNSAFE",
            Self::PackProviderResolutionFailed => "PACK_PROVIDER_RESOLUTION_FAILED",
            Self::PackPlanInvalid => "PACK_PLAN_INVALID",
            Self::PackPlanStale => "PACK_PLAN_STALE",
            Self::PackImportFailed => "PACK_IMPORT_FAILED",
            Self::PackExportInvalid => "PACK_EXPORT_INVALID",
            Self::PackExportStale => "PACK_EXPORT_STALE",
            Self::PackEmbeddingDecisionRequired => "PACK_EMBEDDING_DECISION_REQUIRED",
            Self::PackOutputExists => "PACK_OUTPUT_EXISTS",
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
    Instance,
    Java,
    Launch,
    Authentication,
    Content,
    Modpack,
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
        assert_eq!(
            ErrorCode::ContentProviderUnavailable.as_str(),
            "CONTENT_PROVIDER_UNAVAILABLE"
        );
        assert_eq!(
            ErrorCode::ContentIncompatible.as_str(),
            "CONTENT_INCOMPATIBLE"
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
