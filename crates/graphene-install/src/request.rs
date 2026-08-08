use graphene_core::Result;
use graphene_instance::NewInstanceSpec;
use graphene_minecraft::MinecraftVersionId;

/// Minimal create-only request.
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
