//! Graphene's service composition layer through Phase 2.
//!
//! This crate wires platform, storage, networking, provider adapters, accounts, installation,
//! managed/local Java selection, launch planning, and process execution. Provider DTOs and domain algorithms remain in their
//! owning crates; service owns composition and dependency inversion adapters rather than duplicate
//! business logic.

mod account_service;
mod adapters;
mod artifact_service;
mod builder;
mod context;
mod install_service;
mod java_service;
mod launch_service;
mod minecraft_service;
mod operation_lifecycle;
mod operation_service;

pub use account_service::{AccountService, AccountSessionOperation, MicrosoftLoginOperation};
pub use artifact_service::{
    ArtifactOperation, ArtifactService, DownloadDisposition, VerifiedArtifact,
};
pub use builder::{Graphene, GrapheneBuilder, PlatformInfo};
pub use install_service::{InstallExecutionOperation, InstallPlanOperation, InstallService};
pub use java_service::{JavaService, ManagedJavaOperation};
pub use launch_service::LaunchService;
pub use minecraft_service::{MinecraftManifestOperation, MinecraftService};
pub use operation_service::{OperationService, SyntheticOperation};

pub use graphene_network::{NetworkConfig, ProxyPolicy, RedirectPolicy, RetryPolicy};
pub use graphene_platform::{Architecture, OperatingSystem};
pub use graphene_providers::{
    AdoptiumProviderConfig, MicrosoftAuthConfig, MicrosoftServiceEndpoints, MojangProviderConfig,
};
