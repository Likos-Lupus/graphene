use crate::{error::launch_error, session::LaunchSession};
use graphene_core::{ErrorCode, InstanceId, Result};
use std::path::PathBuf;

const MAX_EXTRA_ARGUMENTS: usize = 256;
const MAX_ARGUMENT_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchResolution {
    pub width: u32,
    pub height: u32,
}

impl LaunchResolution {
    pub fn validate(self) -> Result<Self> {
        if self.width == 0 || self.height == 0 || self.width > 16_384 || self.height > 16_384 {
            return Err(launch_error(
                ErrorCode::LaunchPlanInvalid,
                "launch resolution is outside the supported bound",
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchRequest {
    pub instance_id: InstanceId,
    pub session: LaunchSession,
    pub java_override: Option<PathBuf>,
    pub resolution: Option<LaunchResolution>,
    pub extra_jvm_args: Vec<String>,
    pub extra_game_args: Vec<String>,
}

impl LaunchRequest {
    #[must_use]
    pub fn new(instance_id: InstanceId, session: LaunchSession) -> Self {
        Self {
            instance_id,
            session,
            java_override: None,
            resolution: None,
            extra_jvm_args: Vec::new(),
            extra_game_args: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        self.session.validate()?;

        if let Some(resolution) = self.resolution {
            resolution.validate()?;
        }

        if self.extra_jvm_args.len() > MAX_EXTRA_ARGUMENTS
            || self.extra_game_args.len() > MAX_EXTRA_ARGUMENTS
        {
            return Err(launch_error(
                ErrorCode::LaunchPlanInvalid,
                "too many explicit launch arguments",
            ));
        }

        for value in self.extra_jvm_args.iter().chain(&self.extra_game_args) {
            validate_argument(value)?;
        }

        Ok(())
    }
}

pub(crate) fn validate_argument(value: &str) -> Result<()> {
    if value.len() > MAX_ARGUMENT_BYTES || value.contains('\0') {
        Err(launch_error(
            ErrorCode::LaunchPlanInvalid,
            "launch argument exceeds its bound or contains NUL",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_argument_bounds_reject_nul() {
        assert!(validate_argument("bad\0arg").is_err());
    }
}
