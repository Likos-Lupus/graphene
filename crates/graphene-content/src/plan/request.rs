use crate::{
    id::{ContentEntryId, ContentFileRef, ContentVersionRef},
    model::compatibility::ReleaseChannelPolicy,
};
use graphene_core::{InstanceId, Sha256Digest};
use graphene_instance::ManagedRelativePath;
use serde::{Deserialize, Serialize};

/// An individual action requested as part of a content mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentActionRequest {
    /// Resolves and installs an exact remote version (and its required dependencies).
    InstallExactVersion(ContentVersionRef),
    /// Adopts an already-present recognized local mod file into Graphene managed desired state.
    AdoptRecognizedLocal {
        path: ManagedRelativePath,
        expected_sha256: Sha256Digest,
        file_ref: ContentFileRef,
    },
    /// Updates an existing managed mod to a target version or latest compatible version.
    UpdateManaged {
        entry_id: ContentEntryId,
        target_version: Option<ContentVersionRef>,
    },
    /// Changes the enabled/disabled state of an existing managed mod.
    SetEnabled {
        entry_id: ContentEntryId,
        enabled: bool,
    },
    /// Removes an existing managed mod from desired state and filesystem.
    RemoveManaged {
        entry_id: ContentEntryId,
        cascade: bool,
    },
    /// Removes an exact unmanaged file from the filesystem with fingerprint verification.
    RemoveLocalExact {
        path: ManagedRelativePath,
        expected_sha256: Sha256Digest,
        expected_size: u64,
    },
}

/// Request to plan a batch of content mutations against an instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentMutationRequest {
    pub instance_id: InstanceId,
    pub actions: Vec<ContentActionRequest>,
    pub policy: ReleaseChannelPolicy,
}

impl ContentMutationRequest {
    #[must_use]
    pub fn new(instance_id: InstanceId, actions: Vec<ContentActionRequest>) -> Self {
        Self {
            instance_id,
            actions,
            policy: ReleaseChannelPolicy::default(),
        }
    }

    #[must_use]
    pub fn with_policy(mut self, policy: ReleaseChannelPolicy) -> Self {
        self.policy = policy;
        self.step_policy(policy)
    }

    fn step_policy(mut self, policy: ReleaseChannelPolicy) -> Self {
        self.policy = policy;
        self
    }
}
