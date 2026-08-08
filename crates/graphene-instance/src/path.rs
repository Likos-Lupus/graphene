use crate::error::instance_error;
use graphene_core::Result;
use serde::{Deserialize, Serialize};

pub const MAX_MANAGED_PATH_BYTES: usize = 4096;

/// Persisted managed-relative path. This is intentionally independent of Minecraft domain types.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ManagedRelativePath(String);

impl ManagedRelativePath {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        validate_managed_relative_path(&value)?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub(crate) fn validate_managed_relative_path(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_MANAGED_PATH_BYTES
        || value.contains('\0')
        || value.contains('\\')
    {
        return Err(instance_error("managed relative path is invalid"));
    }

    if value.starts_with('/') || value.starts_with("//") {
        return Err(instance_error("managed relative path must not be absolute"));
    }

    let mut components = value.split('/');

    let Some(first) = components.next() else {
        return Err(instance_error("managed relative path is empty"));
    };

    if first.is_empty() || first == "." || first == ".." || first.contains(':') {
        return Err(instance_error(
            "managed relative path has an unsafe first component",
        ));
    }

    for component in components {
        if component.is_empty() || component == "." || component == ".." || component.contains(':')
        {
            return Err(instance_error(
                "managed relative path contains an unsafe component",
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_paths_reject_escape_forms() {
        for invalid in ["", "/absolute", "../escape", "a/../b", "C:/escape", "a\\b"] {
            assert!(ManagedRelativePath::new(invalid).is_err(), "{invalid}");
        }
        assert!(ManagedRelativePath::new(".graphene/natives/fixture").is_ok());
    }
}
