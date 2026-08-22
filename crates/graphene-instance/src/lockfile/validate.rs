use crate::{
    error::instance_error,
    lockfile::model::{
        InstanceLockfile, LOCKFILE_SCHEMA_VERSION, MAX_LOCKED_ARTIFACTS, MAX_LOCKED_COMPONENTS,
        MAX_LOCKED_EXTRACTIONS, MAX_LOCKED_OUTPUTS,
    },
};
use graphene_core::Result;

impl InstanceLockfile {
    /// Validates lockfile structure, bounds, and paths.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != LOCKFILE_SCHEMA_VERSION {
            return Err(instance_error(
                "instance lockfile schema version is unsupported",
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

        Ok(())
    }
}
