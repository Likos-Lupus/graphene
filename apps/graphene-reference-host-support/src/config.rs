use graphene::{
    AdoptiumProviderConfig, CurseForgeProviderConfig, ErrorCode, ErrorKind, FabricProviderConfig,
    ForgeProviderConfig, Graphene, GrapheneBuilder, GrapheneError, MicrosoftAuthConfig,
    ModrinthProviderConfig, MojangProviderConfig, NeoForgeProviderConfig, NetworkConfig,
    SecretStore, SensitiveString,
};
use std::{env, path::PathBuf, sync::Arc};

/// Host-owned engine configuration resolved before constructing the root `graphene` facade.
///
/// The engine still requires an explicit data root; the host chooses it from an explicit argument,
/// an environment variable, or a platform-appropriate default, but never from a hidden engine
/// process-global singleton.
pub struct HostConfig {
    data_root: PathBuf,
    network: NetworkConfig,
    mojang: MojangProviderConfig,
    fabric: FabricProviderConfig,
    forge: ForgeProviderConfig,
    neoforge: NeoForgeProviderConfig,
    modrinth: ModrinthProviderConfig,
    adoptium: AdoptiumProviderConfig,
    curseforge: Option<CurseForgeProviderConfig>,
    microsoft: Option<MicrosoftAuthConfig>,
    verbosity: u8,
}

impl std::fmt::Debug for HostConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostConfig")
            .field("data_root", &self.data_root)
            .field("verbosity", &self.verbosity)
            .field("curseforge_configured", &self.curseforge.is_some())
            .field("microsoft_configured", &self.microsoft.is_some())
            .finish_non_exhaustive()
    }
}

impl HostConfig {
    /// Creates configuration with production provider defaults and the given explicit data root.
    #[must_use]
    pub fn new(data_root: impl Into<PathBuf>) -> Self {
        Self {
            data_root: data_root.into(),
            network: NetworkConfig::default(),
            mojang: MojangProviderConfig::default(),
            fabric: FabricProviderConfig::default(),
            forge: ForgeProviderConfig::default(),
            neoforge: NeoForgeProviderConfig::default(),
            modrinth: ModrinthProviderConfig::default(),
            adoptium: AdoptiumProviderConfig::default(),
            curseforge: None,
            microsoft: None,
            verbosity: 0,
        }
    }

    /// Resolves configuration from an optional explicit data root plus non-secret environment
    /// settings. Secrets are never read from persisted ordinary config.
    pub fn from_env(data_root: Option<PathBuf>) -> Result<Self, GrapheneError> {
        let mut config = Self::new(resolve_data_root(data_root)?);
        if let Ok(key) = env::var("GRAPHENE_CURSEFORGE_API_KEY")
            && !key.trim().is_empty()
        {
            config = config.with_curseforge_key(Some(SensitiveString::new(key)))?;
        }
        Ok(config)
    }

    #[must_use]
    pub fn data_root(&self) -> &std::path::Path {
        &self.data_root
    }

    #[must_use]
    pub const fn verbosity(&self) -> u8 {
        self.verbosity
    }

    /// Sets the host verbosity level (`0` = warn, `1` = info, `2` = debug, `3+` = trace).
    #[must_use]
    pub const fn with_verbosity(mut self, verbosity: u8) -> Self {
        self.verbosity = verbosity;
        self
    }

    /// Overrides the outbound transport policy.
    #[must_use]
    pub fn with_network(mut self, network: NetworkConfig) -> Self {
        self.network = network;
        self
    }

    /// Installs an optional CurseForge distributor API key from a secret-aware source.
    pub fn with_curseforge_key(
        mut self,
        api_key: Option<SensitiveString>,
    ) -> Result<Self, GrapheneError> {
        self.curseforge = Some(CurseForgeProviderConfig::production(api_key)?);
        Ok(self)
    }

    /// Installs an optional distributor-owned Microsoft application registration.
    #[must_use]
    pub fn with_microsoft(mut self, microsoft: Option<MicrosoftAuthConfig>) -> Self {
        self.microsoft = microsoft;
        self
    }

    /// Builds a configured (not yet created) root builder around the resolved host settings.
    #[must_use]
    pub fn builder(&self, secret_store: Arc<dyn SecretStore>) -> GrapheneBuilder {
        let mut builder = Graphene::builder(self.data_root.clone())
            .network(self.network.clone())
            .minecraft_provider(self.mojang.clone())
            .fabric_provider(self.fabric.clone())
            .forge_provider(self.forge.clone())
            .neoforge_provider(self.neoforge.clone())
            .modrinth_provider(self.modrinth.clone())
            .managed_java_provider(self.adoptium.clone())
            .secret_store(secret_store);

        if let Some(curseforge) = &self.curseforge {
            builder = builder.curseforge_provider(curseforge.clone());
        }

        if let Some(microsoft) = &self.microsoft {
            builder = builder.microsoft_auth(microsoft.clone());
        }

        builder
    }

    /// Constructs the root `graphene` facade from explicit host configuration.
    pub async fn build(
        &self,
        secret_store: Arc<dyn SecretStore>,
    ) -> Result<Graphene, GrapheneError> {
        self.builder(secret_store).build().await
    }
}

/// Resolves the engine data root from an explicit value, `GRAPHENE_DATA_ROOT`, or a platform
/// default. The selected path is always explicit to the engine.
pub fn resolve_data_root(explicit: Option<PathBuf>) -> Result<PathBuf, GrapheneError> {
    if let Some(path) = explicit {
        return Ok(path);
    }

    if let Ok(value) = env::var("GRAPHENE_DATA_ROOT")
        && !value.trim().is_empty()
    {
        return Ok(PathBuf::from(value));
    }

    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or_else(|| {
            GrapheneError::new(
                ErrorCode::DataRootInvalid,
                ErrorKind::Configuration,
                "no data root was supplied and no home directory could be resolved",
            )
        })?;

    Ok(PathBuf::from(home).join(".graphene"))
}
