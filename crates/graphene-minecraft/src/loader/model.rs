use crate::{
    ComponentDescriptor, ComponentVersion, MinecraftVersionId, MinecraftVersionPatch,
    loader::preparation::ComponentPreparationRecipe,
};
use graphene_core::Result;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum LoaderKind {
    Fabric,
    Forge,
    NeoForge,
}

impl LoaderKind {
    #[must_use]
    pub const fn provider_id(self) -> &'static str {
        match self {
            Self::Fabric => "fabric",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
        }
    }

    #[must_use]
    pub const fn component_uid(self) -> &'static str {
        match self {
            Self::Fabric => "net.fabricmc.fabric-loader",
            Self::Forge => "net.minecraftforge.forge",
            Self::NeoForge => "net.neoforged.neoforge",
        }
    }
}

impl fmt::Display for LoaderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.provider_id())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LoaderVersion(pub ComponentVersion);

impl LoaderVersion {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        ComponentVersion::new(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for LoaderVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum LoaderVersionSelector {
    Exact(LoaderVersion),
    LatestStable,
    Recommended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoaderSelection {
    pub kind: LoaderKind,
    pub version: LoaderVersionSelector,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum LoaderSupport {
    Supported,
    MetadataOnly { reason: String },
    Unsupported { reason: String },
}

impl LoaderSupport {
    #[must_use]
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Supported)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct LoaderProviderCapabilities(u32);

impl LoaderProviderCapabilities {
    pub const VERSION_LIST: Self = Self(1 << 0);
    pub const EXACT_RESOLUTION: Self = Self(1 << 1);
    pub const PROFILE_PATCH: Self = Self(1 << 2);
    pub const INSTALLER_ARCHIVE: Self = Self(1 << 3);
    pub const PROCESSOR_RECIPE: Self = Self(1 << 4);
    pub const LEGACY_PROFILE: Self = Self(1 << 5);
    pub const RECOMMENDED_RELEASE: Self = Self(1 << 6);
    pub const CHECKSUM_SIDECARS: Self = Self(1 << 7);

    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoaderVersionSummary {
    pub kind: LoaderKind,
    pub version: LoaderVersion,
    pub minecraft: MinecraftVersionId,
    pub stable: Option<bool>,
    pub recommended: Option<bool>,
    pub release_time: Option<String>,
    pub support: LoaderSupport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedLoader {
    pub kind: LoaderKind,
    pub version: LoaderVersion,
    pub minecraft: MinecraftVersionId,
    pub component: ComponentDescriptor,
    pub patch: MinecraftVersionPatch,
    pub preparation: ComponentPreparationRecipe,
    pub support: LoaderSupport,
}
