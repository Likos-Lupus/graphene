//! Graphene's service composition layer.
//!
//! This crate wires platform, storage, networking, provider adapters, accounts, installation,
//! managed/local Java selection, launch planning, and process execution. Provider DTOs and domain algorithms remain in their
//! owning crates; service owns composition and dependency inversion adapters rather than duplicate
//! business logic.

mod account_service;
mod adapters;
mod artifact_service;
mod builder;
mod component_install;
mod content_service;
mod context;
mod install_service;
mod install_tool_runner;
mod instance_service;
mod java_service;
mod launch_service;
mod loader_service;
mod minecraft_service;
mod modpack_service;
mod operation_lifecycle;
mod operation_service;

pub use account_service::{AccountService, AccountSessionOperation, MicrosoftLoginOperation};
pub use artifact_service::{
    ArtifactOperation, ArtifactService, DownloadDisposition, VerifiedArtifact,
};
pub use builder::{Graphene, GrapheneBuilder, PlatformInfo};
pub use content_service::{
    AvailableContentUpdate, ContentExecuteOperation, ContentFaultPoint, ContentMutationJournal,
    ContentMutationResult, ContentPlanOperation, ContentProjectOperation,
    ContentRecognitionOperation, ContentScanOperation, ContentSearchOperation, ContentService,
    ContentUpdatesOperation, ContentVersionOperation, ContentVersionsOperation,
};
pub use install_service::{InstallExecutionOperation, InstallPlanOperation, InstallService};
pub use instance_service::{
    InstanceCloneOperation, InstanceDeleteOperation, InstanceRepairOperation, InstanceRepository,
    InstanceService, InstanceVerifyOperation,
};
pub use java_service::{JavaService, ManagedJavaOperation};
pub use launch_service::LaunchService;
pub use loader_service::{LoaderResolveOperation, LoaderService, LoaderVersionsOperation};
pub use minecraft_service::{MinecraftManifestOperation, MinecraftService};
pub use modpack_service::{
    ExportEmbeddingPolicy, ModpackExecutionOperation, ModpackExportExecutionOperation,
    ModpackExportPlan, ModpackExportPlanOperation, ModpackExportRequest, ModpackExportResult,
    ModpackImportPlan, ModpackImportRequest, ModpackInspectionOperation, ModpackPlanOperation,
    ModpackService, PackSource,
};
pub use operation_service::{OperationService, SyntheticOperation};

pub use graphene_network::{NetworkConfig, ProxyPolicy, RedirectPolicy, RetryPolicy};
pub use graphene_platform::{Architecture, OperatingSystem};
pub use graphene_providers::{
    AdoptiumProviderConfig, CurseForgeProviderConfig, FabricProviderConfig, ForgeProviderConfig,
    MicrosoftAuthConfig, MicrosoftServiceEndpoints, ModrinthProviderConfig, MojangProviderConfig,
    NeoForgeProviderConfig,
};
