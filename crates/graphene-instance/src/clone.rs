use crate::spec::validate_display_name;
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, InstanceId, Result};
use serde::{Deserialize, Serialize};

/// Mode for cloning an instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CloneMode {
    /// Copies the full instance directory tree including user data (.minecraft), rewriting Graphene identities.
    #[default]
    Full,
    /// Copies only Graphene-managed state and metadata.
    ManagedStateOnly,
}

/// Request to clone an existing instance to a new target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloneRequest {
    pub destination_id: InstanceId,
    pub destination_display_name: String,
    pub mode: CloneMode,
}

impl CloneRequest {
    /// Creates a new full clone request with a generated destination ID.
    pub fn new(destination_display_name: impl Into<String>) -> Result<Self> {
        Self::with_id(InstanceId::new(), destination_display_name, CloneMode::Full)
    }

    /// Creates a clone request with explicit destination ID and mode.
    pub fn with_id(
        destination_id: InstanceId,
        destination_display_name: impl Into<String>,
        mode: CloneMode,
    ) -> Result<Self> {
        let destination_display_name = destination_display_name.into();
        validate_display_name(&destination_display_name)?;
        Ok(Self {
            destination_id,
            destination_display_name,
            mode,
        })
    }

    /// Validates the request against the source instance identifier.
    pub fn validate(&self, source_id: InstanceId) -> Result<()> {
        if self.destination_id == source_id {
            return Err(GrapheneError::new(
                ErrorCode::InstanceCloneFailed,
                ErrorKind::Instance,
                "clone destination ID cannot match source instance ID",
            ));
        }
        validate_display_name(&self.destination_display_name)
    }
}
