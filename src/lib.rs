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
pub use graphene_content::{
    CONTENT_PLAN_SCHEMA_VERSION, CompatibilityResult, ContentActionRequest, ContentDependency,
    ContentEntryId, ContentFile, ContentFileMatch, ContentFileRef, ContentInventoryFingerprint,
    ContentKind, ContentMutationPlan, ContentMutationRequest, ContentProject, ContentProjectRef,
    ContentProvider, ContentProviderCapabilities, ContentProviderFuture, ContentProviderId,
    ContentSearchHit, ContentSearchPage, ContentSearchQuery, ContentSide, ContentVersion,
    ContentVersionFilter, ContentVersionRef, DependencyRelation, DependencyTarget,
    DuplicateModFinding, EnvironmentSupport, ExactMatchStatus, FileMatchItem, FileMatchRequest,
    FileRole, IncompatibilityReason, InstanceContentContext, LocalContentFile,
    LocalContentInventory, LocalFileStatus, LocalModDependency, MAX_PLAN_ACTIONS, MAX_PLAN_ENTRIES,
    MAX_PLAN_FILESYSTEM_ACTIONS, ModMetadataSource, NormalizedModDescriptor, PlannedContentEntry,
    PlannedFilesystemAction, ReleaseChannel, ReleaseChannelPolicy, compute_murmur2,
    sanitize_mod_filename, scan_local_inventory, stream_file_hashes,
};
pub use graphene_core::{
    AccountId, Artifact, ArtifactId, ArtifactIntegrity, ArtifactKind, ArtifactSource, CachePolicy,
    CancellationToken, Diagnostic, DiagnosticCode, DiagnosticParameters, DiagnosticSeverity,
    ErrorCode, ErrorContext, ErrorKind, ErrorSummary, GrapheneError, HashParseError, IdParseError,
    InstanceId, ManagedRuntimeId, OperationController, OperationEvent, OperationEventKind,
    OperationEventStream, OperationHandle, OperationId, OperationResult, OperationSnapshot,
    OperationState, Progress, SensitiveString, Sha1Digest, Sha256Digest,
};
pub use graphene_diagnostics::{
    Confidence, DiagnosticCompleteness, DiagnosticEvidence, DiagnosticFinding, DiagnosticMode,
    DiagnosticRecommendation, DiagnosticReport, DiagnosticRequest, DiagnosticSourcePolicy,
    DiagnosticSourceSummary, DiagnosticVerificationPolicy, EvidenceId, EvidenceSourceKind,
    FindingId, ProcessExitEvidence, RecommendationActionKind,
};
pub use graphene_install::{
    AcquiredArtifact, AcquisitionDisposition, ArtifactAcquirer, ComponentInstallRequest,
    INSTALL_PLAN_VERSION, InstallPlan, InstallRequest, InstallToolRunner, Materialization,
    MaterializationScope, NativeExtraction, PlannedArtifact, PlannedInstance,
    ProcessorExpansionContext, SelectedToolJava, ToolJavaFuture, ToolRunRequest, ToolRunResult,
    ToolRunnerFuture,
};
pub use graphene_instance::{
    CloneMode, CloneRequest, CommittedInstance, DeleteOptions, EffectiveInstanceConfig,
    FindingCode, FindingSeverity, GLOBAL_CONFIG_SCHEMA_VERSION, GlobalInstanceConfig,
    INSTALL_FORMAT_VERSION, INSTALL_RECEIPT_SCHEMA_VERSION, INSTANCE_CONFIG_SCHEMA_VERSION,
    INSTANCE_SCHEMA_VERSION, InstallReceipt, InstalledArgument, InstalledArtifact,
    InstalledComponent, InstalledComponentKind, InstalledJavaRequirement, InstalledLibrary,
    InstalledRule, InstalledRuleAction, InstanceConfig, InstanceConfigPatch, InstanceDescriptor,
    InstanceInventoryEntry, InstanceLockfile, InstanceResolution, InstanceStateFingerprint,
    InstanceStatus, LOCKFILE_SCHEMA_VERSION, LockedArtifact, LockedContentDependency,
    LockedContentEntry, LockedGeneratedOutput, LockedMaterializationScope, LockedNativeExtraction,
    MAX_CONFIG_ARGUMENT_BYTES, MAX_CONFIG_ARGUMENTS, MAX_CONFIG_ENV_ENTRIES, MAX_LOCKED_ARTIFACTS,
    MAX_LOCKED_COMPONENTS, MAX_LOCKED_CONTENT, MAX_LOCKED_DEPENDENCIES_PER_ENTRY,
    MAX_LOCKED_EXTRACTIONS, MAX_LOCKED_OUTPUTS, MAX_VERIFICATION_FINDINGS, ManagedRelativePath,
    MemoryPolicy, NewInstanceSpec, REPAIR_PLAN_SCHEMA_VERSION, RepairAction, RepairOptions,
    RepairPlan, RepairResult, Repairability, SettingUpdate, VerificationFinding, VerificationMode,
    VerificationReport,
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
    Argument, ComponentConflict, ComponentDescriptor, ComponentGraph, ComponentKind,
    ComponentPreparationRecipe, ComponentProvenance, ComponentRequest, ComponentRequirement,
    ComponentUid, ComponentVersion, GeneratedOutput, GeneratedOutputScope, LatestVersions, Library,
    LoaderKind, LoaderProviderCapabilities, LoaderSelection, LoaderSupport, LoaderVersion,
    LoaderVersionSelector, LoaderVersionSummary, ManagedPath, MavenCoordinate, MinecraftArch,
    MinecraftJavaRequirement, MinecraftOs, MinecraftVersionId, MinecraftVersionMetadata,
    MinecraftVersionPatch, MinecraftVersionType, OsRule, PreparationArgument,
    PreparationArgumentPart, PreparationDataValue, PreparationPlaceholder, ProcessorSideCondition,
    ProcessorStep, ResolvedArtifact, ResolvedAssetObject, ResolvedAssets, ResolvedComponent,
    ResolvedLibrary, ResolvedLoader, ResolvedLogging, ResolvedMinecraft, Rule, RuleAction,
    RuleContext, VersionManifest, VersionSummary, loader_support_diagnostic,
};
pub use graphene_modpack::{
    EmbeddedPackFile, FileSelection, NormalizedModpack, OptionalSelectionPolicy, PackArchiveIndex,
    PackDiagnostic, PackDiagnosticCode, PackFormat, PackInspection, PackMetadata,
    PackOptionalChoice, PackRuntimeRequirement, PackSourceSnapshot, PackTargetSide,
    PendingProviderFile, ProviderFileRef, SeedLayer,
};
pub use graphene_service::{
    AccountService, AccountSessionOperation, AdoptiumProviderConfig, Architecture,
    ArtifactOperation, ArtifactService, AvailableContentUpdate, ContentExecuteOperation,
    ContentFaultPoint, ContentMutationJournal, ContentMutationResult, ContentPlanOperation,
    ContentProjectOperation, ContentRecognitionOperation, ContentScanOperation,
    ContentSearchOperation, ContentService, ContentUpdatesOperation, ContentVersionOperation,
    ContentVersionsOperation, CurseForgeProviderConfig, DiagnosticAnalysisOperation,
    DiagnosticService, DownloadDisposition, FabricProviderConfig, ForgeProviderConfig, Graphene,
    GrapheneBuilder, InstallExecutionOperation, InstallPlanOperation, InstallService,
    InstanceCloneOperation, InstanceDeleteOperation, InstanceRepairOperation, InstanceRepository,
    InstanceService, InstanceVerifyOperation, JavaService, LaunchService, LoaderResolveOperation,
    LoaderService, LoaderVersionsOperation, ManagedJavaOperation, MicrosoftAuthConfig,
    MicrosoftLoginOperation, MicrosoftServiceEndpoints, MinecraftManifestOperation,
    MinecraftService, ModpackExecutionOperation, ModpackInspectionOperation, ModpackPlanOperation,
    ModpackService, ModrinthProviderConfig, MojangProviderConfig, NeoForgeProviderConfig,
    NetworkConfig, OperatingSystem, OperationService, PackSource, PlatformInfo, ProxyPolicy,
    RedirectPolicy, RetryPolicy, SyntheticOperation, VerifiedArtifact,
};
