use crate::error::mc_error;
use graphene_core::{ErrorCode, Result};
use serde::{Deserialize, Serialize};

const MAX_PATH_LEN: usize = 1024;

/// Graphene-owned managed relative path encoded with `/` separators.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ManagedPath(String);

impl ManagedPath {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_PATH_LEN || value.contains('\0') {
            return Err(mc_error(
                ErrorCode::MinecraftMetadataInvalid,
                "managed path is empty or exceeds its bound",
            ));
        }

        if value.starts_with('/') || value.starts_with('\\') || value.contains('\\') {
            return Err(mc_error(
                ErrorCode::MinecraftMetadataInvalid,
                "managed path must be a forward-slash relative path",
            ));
        }

        let mut count = 0usize;
        for component in value.split('/') {
            count += 1;
            if component.is_empty() || component == "." || component == ".." {
                return Err(mc_error(
                    ErrorCode::MinecraftMetadataInvalid,
                    "managed path contains an unsafe component",
                ));
            }

            if count == 1 && component.contains(':') {
                return Err(mc_error(
                    ErrorCode::MinecraftMetadataInvalid,
                    "managed path contains a platform prefix",
                ));
            }
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
