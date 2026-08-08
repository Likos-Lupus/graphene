//! Graphene's stable UI-independent launcher-engine facade.
//!
//! The root crate re-exports Graphene-owned Phase 0 foundation values and Phase 1 domain/service
//! entry points. Provider DTOs, HTTP responses, ZIP internals, and Tokio child handles stay behind
//! their bounded-context implementation crates.

pub use graphene_core::{
    Artifact, ArtifactId, ArtifactIntegrity, ArtifactKind, ArtifactSource, CachePolicy,
    CancellationToken, Diagnostic, DiagnosticCode, DiagnosticParameters, DiagnosticSeverity,
    ErrorCode, ErrorContext, ErrorKind, ErrorSummary, GrapheneError, HashParseError, IdParseError,
    InstanceId, OperationController, OperationEvent, OperationEventKind, OperationEventStream,
    OperationHandle, OperationId, OperationResult, OperationSnapshot, OperationState, Progress,
    SensitiveString, Sha1Digest, Sha256Digest,
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
    JavaArchitecture, JavaCandidate, JavaCandidateSource, JavaRequirement, JavaRuntime, JavaVendor,
    discover_java_candidates, is_compatible as java_is_compatible, normalize_vendor,
    parse_java_major, probe_java, select_java,
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
    Architecture, ArtifactOperation, ArtifactService, DownloadDisposition, Graphene,
    GrapheneBuilder, InstallExecutionOperation, InstallPlanOperation, InstallService, JavaService,
    LaunchService, MinecraftManifestOperation, MinecraftService, MojangProviderConfig,
    NetworkConfig, OperatingSystem, OperationService, PlatformInfo, ProxyPolicy, RedirectPolicy,
    RetryPolicy, SyntheticOperation, VerifiedArtifact,
};
