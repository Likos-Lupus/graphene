use crate::{ArtifactService, OperationService, context::ServiceContext};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, OperationRegistry, Result};
use graphene_network::{NetworkClient, NetworkConfig};
use graphene_platform::{Architecture, OperatingSystem, Platform};
use graphene_storage::DataRoot;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::debug;

/// Curated normalized platform information exposed by the facade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlatformInfo {
    pub os: OperatingSystem,
    pub architecture: Architecture,
}

/// Fully initialized UI-independent Graphene engine.
#[derive(Clone)]
pub struct Graphene {
    pub(crate) context: Arc<ServiceContext>,
}

impl std::fmt::Debug for Graphene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Graphene")
            .field("data_root", &self.context.storage.path())
            .field("platform", &self.context.platform)
            .finish_non_exhaustive()
    }
}

impl Graphene {
    /// Starts a builder using an explicit data root.
    #[must_use]
    pub fn builder(data_root: impl Into<PathBuf>) -> GrapheneBuilder {
        GrapheneBuilder::new(data_root)
    }

    /// Returns the canonical explicit data-root path.
    #[must_use]
    pub fn data_root(&self) -> &Path {
        self.context.storage.path()
    }

    /// Returns normalized platform information determined during construction.
    #[must_use]
    pub fn platform(&self) -> PlatformInfo {
        PlatformInfo {
            os: self.context.platform.os,
            architecture: self.context.platform.architecture,
        }
    }

    /// Returns the unified operation service.
    #[must_use]
    pub fn operations(&self) -> OperationService {
        OperationService::new(Arc::clone(&self.context))
    }

    /// Returns the generic verified artifact acquisition service.
    #[must_use]
    pub fn artifacts(&self) -> ArtifactService {
        ArtifactService::new(Arc::clone(&self.context))
    }
}

/// Validating constructor for a fully initialized Graphene engine.
#[derive(Debug, Clone)]
pub struct GrapheneBuilder {
    data_root: PathBuf,
    network: NetworkConfig,
    event_channel_capacity: usize,
}

impl GrapheneBuilder {
    /// Creates a builder with conservative Phase 0 defaults.
    #[must_use]
    pub fn new(data_root: impl Into<PathBuf>) -> Self {
        Self {
            data_root: data_root.into(),
            network: NetworkConfig::default(),
            event_channel_capacity: 256,
        }
    }

    /// Replaces the immutable network policy used by this engine.
    #[must_use]
    pub fn network(mut self, network: NetworkConfig) -> Self {
        self.network = network;
        self
    }

    /// Sets bounded per-subscription operation event capacity.
    #[must_use]
    pub const fn event_channel_capacity(mut self, capacity: usize) -> Self {
        self.event_channel_capacity = capacity;
        self
    }

    /// Validates configuration, initializes storage, and constructs every shared Phase 0 service.
    /// A partially initialized [`Graphene`] is never returned.
    pub async fn build(self) -> Result<Graphene> {
        if !(4..=65_536).contains(&self.event_channel_capacity) {
            return Err(GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                "event channel capacity must be between 4 and 65536",
            )
            .with_context(
                "event_channel_capacity",
                self.event_channel_capacity.to_string(),
            ));
        }

        self.network.validate()?;
        let platform = Platform::current();
        let storage = DataRoot::initialize(&self.data_root)?;
        let network = NetworkClient::new(self.network)?;
        let operations = OperationRegistry::new(self.event_channel_capacity)?;
        debug!(
            module = "graphene-service",
            data_root = %storage.path().display(),
            ?platform,
            "Graphene foundation initialized"
        );

        let context = Arc::new(ServiceContext {
            platform,
            storage,
            network,
            operations,
            artifact_gates: std::sync::Mutex::new(std::collections::HashMap::new()),
        });

        Ok(Graphene { context })
    }
}
