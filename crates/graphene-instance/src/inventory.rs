use crate::InstalledComponent;
use graphene_core::InstanceId;
use serde::{Deserialize, Serialize};

/// High-level lifecycle classification for an instance on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InstanceStatus {
    /// Fully compliant Phase 4 instance with valid descriptor, receipt, and desired-state lockfile.
    Ready,
    /// Pre-Phase-4 legacy instance: valid descriptor and receipt, but missing desired-state lockfile.
    Legacy,
    /// Corrupt or unreadable instance metadata on disk.
    Invalid,
}

/// An entry returned when enumerating instances in a data root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceInventoryEntry {
    pub instance_id: InstanceId,
    pub display_name: Option<String>,
    pub status: InstanceStatus,
    pub minecraft_version: Option<String>,
    pub components: Vec<InstalledComponent>,
    pub error: Option<String>,
}

impl InstanceInventoryEntry {
    /// Constructs a ready entry.
    #[must_use]
    pub fn ready(
        instance_id: InstanceId,
        display_name: String,
        minecraft_version: String,
        components: Vec<InstalledComponent>,
    ) -> Self {
        Self {
            instance_id,
            display_name: Some(display_name),
            status: InstanceStatus::Ready,
            minecraft_version: Some(minecraft_version),
            components,
            error: None,
        }
    }

    /// Constructs a legacy entry.
    #[must_use]
    pub fn legacy(
        instance_id: InstanceId,
        display_name: String,
        minecraft_version: String,
        components: Vec<InstalledComponent>,
    ) -> Self {
        Self {
            instance_id,
            display_name: Some(display_name),
            status: InstanceStatus::Legacy,
            minecraft_version: Some(minecraft_version),
            components,
            error: None,
        }
    }

    /// Constructs an invalid entry with bounded diagnostic context.
    #[must_use]
    pub fn invalid(instance_id: InstanceId, error: impl Into<String>) -> Self {
        Self {
            instance_id,
            display_name: None,
            status: InstanceStatus::Invalid,
            minecraft_version: None,
            components: Vec::new(),
            error: Some(error.into()),
        }
    }
}
