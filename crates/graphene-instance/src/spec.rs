use graphene_core::{ErrorCode, ErrorKind, GrapheneError, InstanceId, Result};

pub const MAX_INSTANCE_NAME_BYTES: usize = 128;

/// User-visible create-only instance request data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewInstanceSpec {
    pub id: InstanceId,
    pub display_name: String,
}

impl NewInstanceSpec {
    pub fn new(display_name: impl Into<String>) -> Result<Self> {
        Self::with_id(InstanceId::new(), display_name)
    }

    pub fn with_id(id: InstanceId, display_name: impl Into<String>) -> Result<Self> {
        let display_name = display_name.into();
        validate_display_name(&display_name)?;
        Ok(Self { id, display_name })
    }

    /// Revalidates request data at a service boundary. Fields remain public for ergonomic host
    /// construction, so consumers must not assume construction-time validation is permanent.
    pub fn validate(&self) -> Result<()> {
        validate_display_name(&self.display_name)
    }
}

pub(crate) fn validate_display_name(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > MAX_INSTANCE_NAME_BYTES || value.contains('\0') {
        return Err(GrapheneError::new(
            ErrorCode::InstallRequestInvalid,
            ErrorKind::Install,
            "instance display name is invalid",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_names_are_bounded() {
        assert!(NewInstanceSpec::new("").is_err());
        assert!(NewInstanceSpec::new("x".repeat(MAX_INSTANCE_NAME_BYTES + 1)).is_err());
    }
}
