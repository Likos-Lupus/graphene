use crate::{NewInstanceSpec, error::instance_error, spec::validate_display_name};
use graphene_core::{InstanceId, Result};
use serde::{Deserialize, Serialize};

pub const INSTANCE_SCHEMA_VERSION: u32 = 1;

/// Stable descriptor persisted at `instance.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceDescriptor {
    pub schema_version: u32,
    pub instance_id: InstanceId,
    pub display_name: String,
}

impl InstanceDescriptor {
    pub fn create(spec: &NewInstanceSpec) -> Self {
        Self {
            schema_version: INSTANCE_SCHEMA_VERSION,
            instance_id: spec.id,
            display_name: spec.display_name.clone(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != INSTANCE_SCHEMA_VERSION {
            return Err(instance_error(
                "instance descriptor schema version is unsupported",
            ));
        }
        validate_display_name(&self.display_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_only_descriptor_is_schema_versioned() {
        let spec =
            NewInstanceSpec::with_id(InstanceId::from_bytes([7; 16]), "Fixture").expect("spec");
        let descriptor = InstanceDescriptor::create(&spec);
        assert_eq!(descriptor.schema_version, 1);
        assert_eq!(descriptor.display_name, "Fixture");
        descriptor.validate().expect("valid");
    }
}
