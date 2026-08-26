use crate::{
    error::instance_error,
    lockfile::model::{
        InstanceLockfile, LOCKFILE_SCHEMA_VERSION, MAX_LOCKED_ARTIFACTS, MAX_LOCKED_COMPONENTS,
        MAX_LOCKED_CONTENT, MAX_LOCKED_DEPENDENCIES_PER_ENTRY, MAX_LOCKED_EXTRACTIONS,
        MAX_LOCKED_OUTPUTS, OLDEST_READABLE_LOCKFILE_SCHEMA_VERSION,
    },
};
use graphene_core::Result;
use std::collections::HashSet;

impl InstanceLockfile {
    /// Validates lockfile structure, bounds, and paths.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version < OLDEST_READABLE_LOCKFILE_SCHEMA_VERSION
            || self.schema_version > LOCKFILE_SCHEMA_VERSION
        {
            return Err(instance_error(
                "instance lockfile schema version is unsupported",
            ));
        }

        if self.schema_version < 2 && !self.content.is_empty() {
            return Err(instance_error(
                "schema version 1 lockfile must not contain managed content entries",
            ));
        }

        if self.components.is_empty() || self.components.len() > MAX_LOCKED_COMPONENTS {
            return Err(instance_error(
                "lockfile component count is outside the supported bounds",
            ));
        }

        if self.artifacts.len() > MAX_LOCKED_ARTIFACTS {
            return Err(instance_error(
                "lockfile artifact count exceeds maximum limit",
            ));
        }

        if self.native_extractions.len() > MAX_LOCKED_EXTRACTIONS {
            return Err(instance_error(
                "lockfile native extraction count exceeds maximum limit",
            ));
        }

        if self.generated_outputs.len() > MAX_LOCKED_OUTPUTS {
            return Err(instance_error(
                "lockfile generated output count exceeds maximum limit",
            ));
        }

        if self.content.len() > MAX_LOCKED_CONTENT {
            return Err(instance_error(
                "lockfile content entry count exceeds maximum limit",
            ));
        }

        for artifact in &self.artifacts {
            if artifact.logical_key.is_empty() || artifact.logical_key.len() > 1024 {
                return Err(instance_error(
                    "lockfile artifact logical key length is invalid",
                ));
            }
            for source in &artifact.sources {
                let url_str = source.url();
                if url_str.contains("token=") || url_str.contains("secret=") {
                    return Err(instance_error(
                        "lockfile artifact source URL contains forbidden credential token",
                    ));
                }
            }
        }

        for output in &self.generated_outputs {
            if output.component_uid.is_empty() || output.component_uid.len() > 256 {
                return Err(instance_error(
                    "generated output component uid length is invalid",
                ));
            }
        }

        let mut seen_entries = HashSet::new();
        let mut seen_destinations = HashSet::new();

        for entry in &self.content {
            if entry.entry_id.is_empty() || entry.entry_id.len() > 64 {
                return Err(instance_error(
                    "lockfile content entry id length is invalid",
                ));
            }
            if !seen_entries.insert(&entry.entry_id) {
                return Err(instance_error(
                    "lockfile contains duplicate content entry id",
                ));
            }
            if !seen_destinations.insert(entry.destination.as_str()) {
                return Err(instance_error(
                    "lockfile contains duplicate content destination path",
                ));
            }
            if entry.kind.is_empty() || entry.kind.len() > 64 {
                return Err(instance_error("lockfile content kind length is invalid"));
            }
            if entry.artifact_logical_key.is_empty() || entry.artifact_logical_key.len() > 1024 {
                return Err(instance_error(
                    "lockfile content artifact logical key is invalid",
                ));
            }
            if entry.dependencies.len() > MAX_LOCKED_DEPENDENCIES_PER_ENTRY {
                return Err(instance_error(
                    "lockfile content entry dependency count exceeds maximum limit",
                ));
            }
        }

        if let Some(origin) = &self.pack_origin {
            origin.validate()?;
        }

        Ok(())
    }
}
