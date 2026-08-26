use crate::error::PackError;
use graphene_minecraft::LoaderKind;
use serde::{Deserialize, Serialize};

/// Exact runtime requirement extracted from a pack manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackRuntimeRequirement {
    minecraft_version: String,
    primary_loader: Option<PackLoaderRequirement>,
}

/// Exact primary loader identity; versions are never moving aliases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackLoaderRequirement {
    kind: LoaderKind,
    version: String,
}

impl PackLoaderRequirement {
    pub fn new(kind: LoaderKind, version: impl Into<String>) -> Result<Self, PackError> {
        let version = version.into();
        if version.is_empty() || version.len() > 64 || version.chars().any(char::is_control) {
            return Err(crate::error::PackError::manifest(
                "loader version is missing or unbounded",
            ));
        }
        Ok(Self { kind, version })
    }

    #[must_use]
    pub const fn kind(&self) -> LoaderKind {
        self.kind
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }
}

impl PackRuntimeRequirement {
    pub fn new(
        minecraft_version: impl Into<String>,
        primary_loader: Option<PackLoaderRequirement>,
    ) -> Result<Self, crate::error::PackError> {
        let minecraft_version = minecraft_version.into();
        let valid = !minecraft_version.is_empty()
            && minecraft_version.len() <= 32
            && minecraft_version
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+' | '~'))
            && minecraft_version
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit());
        if !valid {
            return Err(PackError::manifest(
                "pack declares an invalid Minecraft version",
            ));
        }
        Ok(Self {
            minecraft_version,
            primary_loader,
        })
    }

    #[must_use]
    pub fn minecraft_version(&self) -> &str {
        &self.minecraft_version
    }

    #[must_use]
    pub const fn primary_loader(&self) -> Option<&PackLoaderRequirement> {
        self.primary_loader.as_ref()
    }
}
