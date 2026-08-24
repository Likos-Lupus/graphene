use crate::model::{dependency::DependencyRelation, version::EnvironmentSupport};
use serde::{Deserialize, Serialize};

/// Source format where local mod metadata was discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModMetadataSource {
    Fabric,
    Forge,
    NeoForge,
    LegacyMcModInfo,
    LegacyManifest,
    #[default]
    Unknown,
}

/// Normalized local dependency constraint declared in a mod's metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalModDependency {
    pub mod_id: String,
    pub version_range: Option<String>,
    pub relation: DependencyRelation,
}

/// Normalized logical mod descriptor extracted from a JAR archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedModDescriptor {
    pub mod_id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub environment: EnvironmentSupport,
    pub dependencies: Vec<LocalModDependency>,
    pub source: ModMetadataSource,
}

impl NormalizedModDescriptor {
    #[must_use]
    pub fn new(
        mod_id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        source: ModMetadataSource,
    ) -> Self {
        let mod_id = mod_id.into();
        let name = name.into();
        let version = version.into();
        Self {
            mod_id,
            name,
            version,
            description: None,
            authors: Vec::new(),
            environment: EnvironmentSupport::Both,
            dependencies: Vec::new(),
            source,
        }
    }
}
