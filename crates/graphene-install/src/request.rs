use graphene_core::Result;
use graphene_instance::NewInstanceSpec;
use graphene_minecraft::{LoaderSelection, MinecraftVersionId};

/// Vanilla create-only installation request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRequest {
    pub instance: NewInstanceSpec,
    pub minecraft_version: MinecraftVersionId,
}

impl InstallRequest {
    pub fn new(
        display_name: impl Into<String>,
        minecraft_version: impl Into<String>,
    ) -> Result<Self> {
        Ok(Self {
            instance: NewInstanceSpec::new(display_name)?,
            minecraft_version: MinecraftVersionId::new(minecraft_version)?,
        })
    }

    pub fn with_instance(instance: NewInstanceSpec, minecraft_version: MinecraftVersionId) -> Self {
        Self {
            instance,
            minecraft_version,
        }
    }
}

/// Component-aware create request. Dynamic loader selectors are resolved before an InstallPlan is returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentInstallRequest {
    pub instance: NewInstanceSpec,
    pub minecraft_version: MinecraftVersionId,
    pub loader: LoaderSelection,
}

impl ComponentInstallRequest {
    pub fn new(
        display_name: impl Into<String>,
        minecraft_version: impl Into<String>,
        loader: LoaderSelection,
    ) -> Result<Self> {
        Ok(Self {
            instance: NewInstanceSpec::new(display_name)?,
            minecraft_version: MinecraftVersionId::new(minecraft_version)?,
            loader,
        })
    }

    #[must_use]
    pub fn with_instance(
        instance: NewInstanceSpec,
        minecraft_version: MinecraftVersionId,
        loader: LoaderSelection,
    ) -> Self {
        Self {
            instance,
            minecraft_version,
            loader,
        }
    }
}
