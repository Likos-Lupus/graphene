//! Graphene's stable UI-independent launcher-engine facade.
//!
//! The root crate re-exports Graphene-owned foundation, installation/launch, account/authentication,
//! and managed-Java domain/service entry points. Provider DTOs, HTTP responses, ZIP internals, and Tokio child handles stay behind
//! their bounded-context implementation crates.

pub use graphene_auth::{
    Account, AccountKind, AccountProfile, AccountState, AuthInteraction, AuthProviderCapabilities,
    AuthSession, OfflineAccountSpec, RefreshCredential, SecretRecordIdentity, SecretRecordKind,
    SecretStore, account_diagnostic, authentication_error_diagnostic,
};
pub use graphene_core::{
    AccountId, Artifact, ArtifactId, ArtifactIntegrity, ArtifactKind, ArtifactSource, CachePolicy,
    CancellationToken, Diagnostic, DiagnosticCode, DiagnosticParameters, DiagnosticSeverity,
    ErrorCode, ErrorContext, ErrorKind, ErrorSummary, GrapheneError, HashParseError, IdParseError,
    InstanceId, ManagedRuntimeId, OperationController, OperationEvent, OperationEventKind,
    OperationEventStream, OperationHandle, OperationId, OperationResult, OperationSnapshot,
    OperationState, Progress, SensitiveString, Sha1Digest, Sha256Digest,
};
pub use graphene_install::{
    AcquiredArtifact, AcquisitionDisposition, ArtifactAcquirer, INSTALL_PLAN_VERSION, InstallPlan,
    InstallRequest, Materialization, MaterializationScope, NativeExtraction, PlannedArtifact,
    PlannedInstance,
};
pub use graphene_instance::{
    CommittedInstance, INSTALL_FORMAT_VERSION, INSTALL_RECEIPT_SCHEMA_VERSION,
    INSTANCE_SCHEMA_VERSION, InstallReceipt, InstalledArgument, InstalledArtifact,
    InstalledJavaRequirement, InstalledLibrary, InstalledRule, InstalledRuleAction,
    InstanceDescriptor, ManagedRelativePath, NewInstanceSpec,
};
pub use graphene_java::{
    JavaArchitecture, JavaCandidate, JavaCandidateSource, JavaDistributionCapabilities,
    JavaImageKind, JavaOperatingSystem, JavaRequirement, JavaRuntime, JavaVendor,
    MANAGED_RUNTIME_SCHEMA_VERSION, ManagedArchiveFormat, ManagedJavaInstallPlan,
    ManagedJavaRelease, ManagedJavaRequest, ManagedJavaRuntime, compatibility_diagnostic,
    discover_java_candidates, is_compatible as java_is_compatible, normalize_vendor,
    parse_java_major, probe_java, select_java, select_managed_runtime,
};
pub use graphene_launch::{
    EnvironmentDelta, GameEvent, GameEventStream, GameExit, LaunchArgument, LaunchPlan,
    LaunchRequest, LaunchResolution, LaunchSession, RedactedLaunchPlan, RunningGame,
};
pub use graphene_minecraft::{
    Argument, LatestVersions, Library, ManagedPath, MavenCoordinate, MinecraftArch,
    MinecraftJavaRequirement, MinecraftOs, MinecraftVersionId, MinecraftVersionMetadata,
    MinecraftVersionType, OsRule, ResolvedArtifact, ResolvedAssetObject, ResolvedAssets,
    ResolvedLibrary, ResolvedLogging, ResolvedMinecraft, Rule, RuleAction, RuleContext,
    VersionManifest, VersionSummary,
};
pub use graphene_service::{
    AccountService, AccountSessionOperation, AdoptiumProviderConfig, Architecture,
    ArtifactOperation, ArtifactService, DownloadDisposition, Graphene, GrapheneBuilder,
    InstallExecutionOperation, InstallPlanOperation, InstallService, JavaService, LaunchService,
    ManagedJavaOperation, MicrosoftAuthConfig, MicrosoftLoginOperation, MicrosoftServiceEndpoints,
    MinecraftManifestOperation, MinecraftService, MojangProviderConfig, NetworkConfig,
    OperatingSystem, OperationService, PlatformInfo, ProxyPolicy, RedirectPolicy, RetryPolicy,
    SyntheticOperation, VerifiedArtifact,
};
