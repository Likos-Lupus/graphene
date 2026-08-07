//! Minimal Phase 0 composition layer for Graphene.
//!
//! This crate wires platform, storage, networking, and the operation runtime. It deliberately owns
//! no provider- or product-specific behavior.

mod artifact_service;
mod builder;
mod context;
mod operation_service;

pub use artifact_service::{
    ArtifactOperation, ArtifactService, DownloadDisposition, VerifiedArtifact,
};
pub use builder::{Graphene, GrapheneBuilder, PlatformInfo};
pub use operation_service::{OperationService, SyntheticOperation};

pub use graphene_network::{NetworkConfig, ProxyPolicy, RedirectPolicy, RetryPolicy};
pub use graphene_platform::{Architecture, OperatingSystem};
