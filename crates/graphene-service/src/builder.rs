use crate::{
    AccountService, ArtifactService, InstallService, JavaService, LaunchService, LoaderService,
    MinecraftService, OperationService, account_service::StorageAccountRepository,
    context::ServiceContext,
};
use graphene_auth::{SecretStore, UnavailableSecretStore};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, OperationRegistry, Result};
use graphene_network::{NetworkClient, NetworkConfig};
use graphene_platform::{Architecture, OperatingSystem, Platform};
use graphene_providers::{
    AdoptiumProvider, AdoptiumProviderConfig, FabricProvider, FabricProviderConfig, ForgeProvider,
    ForgeProviderConfig, LoaderProviderRegistry, MicrosoftAuthConfig, MicrosoftAuthProvider,
    MojangProviderConfig, NeoForgeProvider, NeoForgeProviderConfig,
};
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

    /// Returns account/authentication orchestration.
    #[must_use]
    pub fn accounts(&self) -> AccountService {
        AccountService::new(Arc::clone(&self.context))
    }

    /// Returns the generic verified artifact acquisition service.
    #[must_use]
    pub fn artifacts(&self) -> ArtifactService {
        ArtifactService::new(Arc::clone(&self.context))
    }

    /// Returns the official-compatible Minecraft metadata service.
    #[must_use]
    pub fn minecraft(&self) -> MinecraftService {
        MinecraftService::new(Arc::clone(&self.context))
    }

    /// Returns normalized loader discovery and exact-resolution services.
    #[must_use]
    pub fn loaders(&self) -> LoaderService {
        LoaderService::new(Arc::clone(&self.context))
    }

    /// Returns deterministic install planning/execution.
    #[must_use]
    pub fn install(&self) -> InstallService {
        InstallService::new(Arc::clone(&self.context))
    }

    /// Returns local and managed Java selection/installation services.
    #[must_use]
    pub fn java(&self) -> JavaService {
        JavaService::new(Arc::clone(&self.context))
    }

    /// Returns offline launch planning and direct process execution.
    #[must_use]
    pub fn launch(&self) -> LaunchService {
        LaunchService::new(Arc::clone(&self.context))
    }

    /// Returns the instance inventory, configuration, verification, and lifecycle service.
    #[must_use]
    pub fn instances(&self) -> crate::instance_service::InstanceService {
        crate::instance_service::InstanceService::new(Arc::clone(&self.context))
    }
}

/// Validating constructor for a fully initialized Graphene engine.
#[derive(Clone)]
pub struct GrapheneBuilder {
    data_root: PathBuf,
    network: NetworkConfig,
    event_channel_capacity: usize,
    provider_config: MojangProviderConfig,
    microsoft_auth: Option<MicrosoftAuthConfig>,
    secret_store: Arc<dyn SecretStore>,
    java_provider_config: AdoptiumProviderConfig,
    fabric_provider_config: FabricProviderConfig,
    forge_provider_config: ForgeProviderConfig,
    neoforge_provider_config: NeoForgeProviderConfig,
}

impl std::fmt::Debug for GrapheneBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GrapheneBuilder")
            .field("data_root", &self.data_root)
            .field("network", &self.network)
            .field("event_channel_capacity", &self.event_channel_capacity)
            .field("provider_config", &self.provider_config)
            .field("microsoft_auth", &self.microsoft_auth)
            .field("secret_store", &"<configured-backend>")
            .field("java_provider_config", &self.java_provider_config)
            .field("fabric_provider_config", &self.fabric_provider_config)
            .field("forge_provider_config", &self.forge_provider_config)
            .field("neoforge_provider_config", &self.neoforge_provider_config)
            .finish()
    }
}

impl GrapheneBuilder {
    /// Creates a builder with conservative foundation defaults.
    #[must_use]
    pub fn new(data_root: impl Into<PathBuf>) -> Self {
        Self {
            data_root: data_root.into(),
            network: NetworkConfig::default(),
            event_channel_capacity: 256,
            provider_config: MojangProviderConfig::default(),
            microsoft_auth: None,
            secret_store: Arc::new(UnavailableSecretStore),
            java_provider_config: AdoptiumProviderConfig::default(),
            fabric_provider_config: FabricProviderConfig::default(),
            forge_provider_config: ForgeProviderConfig::default(),
            neoforge_provider_config: NeoForgeProviderConfig::default(),
        }
    }

    /// Replaces the immutable network policy used by this engine.
    #[must_use]
    pub fn network(mut self, network: NetworkConfig) -> Self {
        self.network = network;
        self
    }

    /// Replaces the narrow Mojang endpoint configuration. Production callers should normally keep
    /// the official HTTPS default; fixture configuration exists for deterministic local tests.
    #[must_use]
    pub fn minecraft_provider(mut self, provider_config: MojangProviderConfig) -> Self {
        self.provider_config = provider_config;
        self
    }

    /// Replaces the Fabric loader provider configuration.
    #[must_use]
    pub fn fabric_provider(mut self, config: FabricProviderConfig) -> Self {
        self.fabric_provider_config = config;
        self
    }

    /// Replaces the Forge loader provider configuration.
    #[must_use]
    pub fn forge_provider(mut self, config: ForgeProviderConfig) -> Self {
        self.forge_provider_config = config;
        self
    }

    /// Replaces the NeoForge loader provider configuration.
    #[must_use]
    pub fn neoforge_provider(mut self, config: NeoForgeProviderConfig) -> Self {
        self.neoforge_provider_config = config;
        self
    }

    /// Configures a distributor-owned Microsoft application and provider endpoints.
    #[must_use]
    pub fn microsoft_auth(mut self, config: MicrosoftAuthConfig) -> Self {
        self.microsoft_auth = Some(config);
        self
    }

    /// Injects an explicit secure credential backend. No plaintext fallback is enabled implicitly.
    #[must_use]
    pub fn secret_store(mut self, store: Arc<dyn SecretStore>) -> Self {
        self.secret_store = store;
        self
    }

    /// Replaces the managed-Java reference-provider configuration.
    #[must_use]
    pub fn managed_java_provider(mut self, config: AdoptiumProviderConfig) -> Self {
        self.java_provider_config = config;
        self
    }

    /// Sets bounded per-subscription operation event capacity.
    #[must_use]
    pub const fn event_channel_capacity(mut self, capacity: usize) -> Self {
        self.event_channel_capacity = capacity;
        self
    }

    /// Validates configuration, initializes storage, and constructs the shared foundation services.
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
        self.provider_config.validate()?;
        if let Some(config) = &self.microsoft_auth {
            config.validate()?;
        }
        self.java_provider_config.validate()?;

        let platform = Platform::current();
        let storage = DataRoot::initialize(&self.data_root)?;
        let network = NetworkClient::new(self.network)?;
        let operations = OperationRegistry::new(self.event_channel_capacity)?;
        let account_repository = Arc::new(StorageAccountRepository::new(&storage)?);
        let auth_provider = match self.microsoft_auth {
            Some(config) => Some(
                Arc::new(MicrosoftAuthProvider::new(network.clone(), config)?)
                    as Arc<dyn graphene_auth::AuthProvider>,
            ),
            None => None,
        };
        let java_distribution_provider = Arc::new(AdoptiumProvider::new(
            network.clone(),
            self.java_provider_config,
        )?)
            as Arc<dyn graphene_java::JavaDistributionProvider>;
        let mut loader_registry = LoaderProviderRegistry::new();

        loader_registry.register(Arc::new(FabricProvider::new(
            network.clone(),
            self.fabric_provider_config,
        )?))?;
        loader_registry.register(Arc::new(ForgeProvider::new(
            network.clone(),
            self.forge_provider_config,
        )?))?;
        loader_registry.register(Arc::new(NeoForgeProvider::new(
            network.clone(),
            self.neoforge_provider_config,
        )?))?;

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
            provider_config: self.provider_config,
            loader_registry,
            account_repository,
            secret_store: self.secret_store,
            auth_provider,
            java_distribution_provider,
            artifact_gates: std::sync::Mutex::new(std::collections::HashMap::new()),
            account_gates: std::sync::Mutex::new(std::collections::HashMap::new()),
            runtime_gates: std::sync::Mutex::new(std::collections::HashMap::new()),
        });

        Ok(Graphene { context })
    }
}
