use crate::error::mc_error;
use graphene_core::{ErrorCode, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_COMPONENT_UID_BYTES: usize = 128;
pub const MAX_COMPONENT_VERSION_BYTES: usize = 192;
pub const MAX_COMPONENT_PROVIDER_BYTES: usize = 96;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ComponentUid(String);

impl ComponentUid {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_COMPONENT_UID_BYTES
            || value.starts_with('.')
            || value.ends_with('.')
            || value.contains("..")
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        {
            return Err(mc_error(
                ErrorCode::ComponentInvalid,
                "component UID is invalid",
            ));
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ComponentUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ComponentVersion(String);

impl ComponentVersion {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.trim().is_empty()
            || value.len() > MAX_COMPONENT_VERSION_BYTES
            || value.chars().any(char::is_control)
            || value.contains('/')
            || value.contains('\\')
        {
            return Err(mc_error(
                ErrorCode::ComponentInvalid,
                "component version is invalid",
            ));
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ComponentVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ComponentKind {
    Minecraft,
    Loader,
    Auxiliary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ComponentRequirement {
    Any {
        uid: ComponentUid,
    },
    Exact {
        uid: ComponentUid,
        version: ComponentVersion,
    },
}

impl ComponentRequirement {
    #[must_use]
    pub fn uid(&self) -> &ComponentUid {
        match self {
            Self::Any { uid } | Self::Exact { uid, .. } => uid,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentConflict {
    pub uid: ComponentUid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentDescriptor {
    pub uid: ComponentUid,
    pub version: ComponentVersion,
    pub kind: ComponentKind,
    pub order: i32,
    pub requires: Vec<ComponentRequirement>,
    pub conflicts: Vec<ComponentConflict>,
}

impl ComponentDescriptor {
    pub fn minecraft(version: impl Into<String>) -> Result<Self> {
        Ok(Self {
            uid: ComponentUid::new("net.minecraft")?,
            version: ComponentVersion::new(version)?,
            kind: ComponentKind::Minecraft,
            order: 0,
            requires: Vec::new(),
            conflicts: Vec::new(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentRequest {
    pub uid: ComponentUid,
    pub version: Option<ComponentVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentProvenance {
    pub provider: String,
    pub detail: Option<String>,
}

impl ComponentProvenance {
    pub fn new(provider: impl Into<String>, detail: Option<String>) -> Result<Self> {
        let provider = provider.into();
        if provider.trim().is_empty()
            || provider.len() > MAX_COMPONENT_PROVIDER_BYTES
            || provider.chars().any(char::is_control)
        {
            return Err(mc_error(
                ErrorCode::ComponentInvalid,
                "component provenance provider is invalid",
            ));
        }

        if detail
            .as_ref()
            .is_some_and(|value| value.len() > 512 || value.chars().any(char::is_control))
        {
            return Err(mc_error(
                ErrorCode::ComponentInvalid,
                "component provenance detail is invalid",
            ));
        }

        Ok(Self { provider, detail })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedComponent {
    pub uid: ComponentUid,
    pub version: ComponentVersion,
    pub kind: ComponentKind,
    pub provenance: ComponentProvenance,
}

impl ResolvedComponent {
    pub fn minecraft(version: impl Into<String>) -> Result<Self> {
        Ok(Self {
            uid: ComponentUid::new("net.minecraft")?,
            version: ComponentVersion::new(version)?,
            kind: ComponentKind::Minecraft,
            provenance: ComponentProvenance::new("mojang", None)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_uid_rejects_path_control_and_ambiguous_forms() {
        for value in ["", ".hidden", "trailing.", "a..b", "a/b", "a\\b", "a\nb"] {
            assert!(ComponentUid::new(value).is_err(), "{value:?}");
        }
        assert!(ComponentUid::new("net.fabricmc.fabric-loader").is_ok());
    }

    #[test]
    fn component_version_is_opaque_but_bounded_and_path_safe() {
        for value in ["", "   ", "a/b", "a\\b", "line\nbreak"] {
            assert!(ComponentVersion::new(value).is_err(), "{value:?}");
        }

        for value in ["0.16.14", "47.3.7-beta", "21.1.207", "1.0+provider-tag"] {
            assert_eq!(
                ComponentVersion::new(value)
                    .expect("opaque version")
                    .as_str(),
                value
            );
        }
    }
}
