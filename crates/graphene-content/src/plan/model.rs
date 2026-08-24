use crate::{
    id::{ContentEntryId, ContentProviderId},
    local::fingerprint::ContentInventoryFingerprint,
    model::{
        compatibility::InstanceContentContext, dependency::ContentDependency, file::ContentFile,
        kind::ContentKind,
    },
    plan::request::ContentActionRequest,
};
use graphene_core::{InstanceId, Sha256Digest};
use graphene_instance::{InstanceStateFingerprint, LockedContentEntry, ManagedRelativePath};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const CONTENT_PLAN_SCHEMA_VERSION: u32 = 1;
pub const MAX_PLAN_ACTIONS: usize = 256;
pub const MAX_PLAN_ENTRIES: usize = 1024;
pub const MAX_PLAN_FILESYSTEM_ACTIONS: usize = 2048;

/// Planned content entry representing an item in the desired state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedContentEntry {
    pub entry_id: ContentEntryId,
    pub kind: ContentKind,
    pub provider: Option<ContentProviderId>,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub file_id: Option<String>,
    pub artifact_logical_key: String,
    pub destination: ManagedRelativePath,
    pub enabled: bool,
    pub dependencies: Vec<ContentDependency>,
}

impl PlannedContentEntry {
    #[must_use]
    pub fn to_locked_entry(&self) -> LockedContentEntry {
        LockedContentEntry {
            entry_id: self.entry_id.as_str().to_string(),
            kind: match self.kind {
                ContentKind::Mod => "MOD".to_string(),
            },
            provider: self.provider.as_ref().map(|p| p.as_str().to_string()),
            project_id: self.project_id.clone(),
            version_id: self.version_id.clone(),
            file_id: self.file_id.clone(),
            artifact_logical_key: self.artifact_logical_key.clone(),
            destination: self.destination.clone(),
            enabled: self.enabled,
            dependencies: self
                .dependencies
                .iter()
                .map(|d| graphene_instance::LockedContentDependency {
                    target: match &d.target {
                        crate::model::dependency::DependencyTarget::Version(v) => v.to_string(),
                        crate::model::dependency::DependencyTarget::Project(p) => p.to_string(),
                        crate::model::dependency::DependencyTarget::FilenameHint(h) => h.clone(),
                    },
                    relation: format!("{:?}", d.relation),
                })
                .collect(),
        }
    }
}

/// A specific filesystem action planned for execution under the exclusive instance lease.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedFilesystemAction {
    /// Stages an acquired artifact from the cache to instance staging.
    StageArtifact {
        artifact_logical_key: String,
        destination: ManagedRelativePath,
    },
    /// Publishes a staged artifact to its final destination path in the instance.
    PublishStagedArtifact {
        staged_relative: String,
        final_destination: ManagedRelativePath,
    },
    /// Quarantines an existing / replaced file before commit.
    QuarantineFile { path: ManagedRelativePath },
    /// Atomically renames a file (e.g. for enable/disable).
    RenameFile {
        from: ManagedRelativePath,
        to: ManagedRelativePath,
    },
    /// Adopts an existing unmanaged file into desired state without copying.
    AdoptExistingFile { path: ManagedRelativePath },
    /// Deletes a file that was quarantined during this transaction.
    DeleteQuarantinedFile { path: ManagedRelativePath },
}

/// A complete, inspectable, deterministic, non-mutating plan of content operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentMutationPlan {
    pub schema_version: u32,
    pub instance_id: InstanceId,
    pub base_state_fingerprint: InstanceStateFingerprint,
    pub base_inventory_fingerprint: ContentInventoryFingerprint,
    pub context: InstanceContentContext,
    pub requested_actions: Vec<ContentActionRequest>,
    pub planned_entries: Vec<PlannedContentEntry>,
    pub filesystem_actions: Vec<PlannedFilesystemAction>,
    pub artifacts_to_acquire: Vec<ContentFile>,
    pub resulting_lockfile_entries: Vec<LockedContentEntry>,
    pub diagnostics: Vec<String>,
    pub estimated_download_bytes: u64,
}

impl ContentMutationPlan {
    /// Returns whether this plan requires changes to the instance state.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.filesystem_actions.is_empty() && self.artifacts_to_acquire.is_empty()
    }
}

/// Derives a safe filename for a mod within `.minecraft/mods`.
#[must_use]
pub fn sanitize_mod_filename(
    provider_filename: &str,
    project_slug_or_id: &str,
    sha256: Option<&Sha256Digest>,
    enabled: bool,
) -> String {
    let contains_path_separators =
        provider_filename.contains('/') || provider_filename.contains('\\');

    let basename = Path::new(provider_filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(provider_filename);

    let is_safe = !contains_path_separators
        && !basename.is_empty()
        && basename.len() <= 128
        && !basename.starts_with('.')
        && !basename.chars().any(|c| {
            c.is_control()
                || c == ':'
                || c == '*'
                || c == '?'
                || c == '"'
                || c == '<'
                || c == '>'
                || c == '|'
        });

    let base_name_without_ext = if is_safe && basename.ends_with(".jar") {
        basename.trim_end_matches(".jar")
    } else if is_safe && basename.ends_with(".jar.disabled") {
        basename.trim_end_matches(".jar.disabled")
    } else {
        // Unsafe name fallback: use project slug + hash
        let hash_part = if let Some(h) = sha256 {
            let hex_str = h.to_string();
            hex_str[..8].to_string()
        } else {
            "mod".to_string()
        };
        let clean_slug: String = project_slug_or_id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        return if enabled {
            format!("{clean_slug}-{hash_part}.jar")
        } else {
            format!("{clean_slug}-{hash_part}.jar.disabled")
        };
    };

    if enabled {
        format!("{base_name_without_ext}.jar")
    } else {
        format!("{base_name_without_ext}.jar.disabled")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_sanitization() {
        assert_eq!(
            sanitize_mod_filename("sodium-fabric-0.5.8.jar", "sodium", None, true),
            "sodium-fabric-0.5.8.jar"
        );
        assert_eq!(
            sanitize_mod_filename("sodium-fabric-0.5.8.jar", "sodium", None, false),
            "sodium-fabric-0.5.8.jar.disabled"
        );
        assert_eq!(
            sanitize_mod_filename("../../etc/passwd.jar", "evil", None, true),
            "evil-mod.jar"
        );
    }
}
