use crate::config::setting::SettingUpdate;
use crate::error::instance_error;
use graphene_core::{AccountId, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const GLOBAL_CONFIG_SCHEMA_VERSION: u32 = 1;
pub const INSTANCE_CONFIG_SCHEMA_VERSION: u32 = 1;

pub const MAX_CONFIG_ARGUMENTS: usize = 256;
pub const MAX_CONFIG_ARGUMENT_BYTES: usize = 32 * 1024;
pub const MAX_CONFIG_ENV_ENTRIES: usize = 128;

/// Game window dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceResolution {
    pub width: u32,
    pub height: u32,
}

impl InstanceResolution {
    /// Validates window resolution bounds.
    pub fn validate(self) -> Result<Self> {
        if self.width == 0 || self.height == 0 || self.width > 16_384 || self.height > 16_384 {
            return Err(instance_error(
                "instance resolution dimensions are outside the supported bounds (1-16384)",
            ));
        }
        Ok(self)
    }
}

/// Typed Java virtual machine memory policy in mebibytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryPolicy {
    pub min_memory_mib: Option<u32>,
    pub max_memory_mib: Option<u32>,
}

impl MemoryPolicy {
    /// Validates memory bounds.
    pub fn validate(self) -> Result<Self> {
        if let (Some(min), Some(max)) = (self.min_memory_mib, self.max_memory_mib)
            && min > max
        {
            return Err(instance_error(
                "minimum memory cannot exceed maximum memory",
            ));
        }
        Ok(self)
    }
}

/// Global instance defaults persisted at `config/instance-defaults.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalInstanceConfig {
    pub schema_version: u32,
    pub java_override: Option<String>,
    pub resolution: Option<InstanceResolution>,
    pub memory: Option<MemoryPolicy>,
    pub jvm_args: Vec<String>,
    pub game_args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub account_id: Option<AccountId>,
}

impl Default for GlobalInstanceConfig {
    fn default() -> Self {
        Self {
            schema_version: GLOBAL_CONFIG_SCHEMA_VERSION,
            java_override: None,
            resolution: None,
            memory: None,
            jvm_args: Vec::new(),
            game_args: Vec::new(),
            env: BTreeMap::new(),
            account_id: None,
        }
    }
}

impl GlobalInstanceConfig {
    /// Validates global defaults.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != GLOBAL_CONFIG_SCHEMA_VERSION {
            return Err(instance_error(
                "global instance defaults schema version is unsupported",
            ));
        }
        if let Some(res) = self.resolution {
            res.validate()?;
        }
        if let Some(mem) = self.memory {
            mem.validate()?;
        }
        validate_arguments(&self.jvm_args, "jvm_args")?;
        validate_arguments(&self.game_args, "game_args")?;
        validate_environment(&self.env)?;
        Ok(())
    }
}

/// Per-instance configuration overrides persisted at `instances/<id>/.graphene/config.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceConfig {
    pub schema_version: u32,
    pub java_override: Option<String>,
    pub resolution: Option<InstanceResolution>,
    pub memory: Option<MemoryPolicy>,
    pub jvm_args: Option<Vec<String>>,
    pub game_args: Option<Vec<String>>,
    pub env: Option<BTreeMap<String, String>>,
    pub account_id: Option<AccountId>,
}

impl Default for InstanceConfig {
    fn default() -> Self {
        Self {
            schema_version: INSTANCE_CONFIG_SCHEMA_VERSION,
            java_override: None,
            resolution: None,
            memory: None,
            jvm_args: None,
            game_args: None,
            env: None,
            account_id: None,
        }
    }
}

impl InstanceConfig {
    /// Validates per-instance overrides.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != INSTANCE_CONFIG_SCHEMA_VERSION {
            return Err(instance_error(
                "instance config schema version is unsupported",
            ));
        }
        if let Some(res) = self.resolution {
            res.validate()?;
        }
        if let Some(mem) = self.memory {
            mem.validate()?;
        }
        if let Some(jvm) = &self.jvm_args {
            validate_arguments(jvm, "jvm_args")?;
        }
        if let Some(game) = &self.game_args {
            validate_arguments(game, "game_args")?;
        }
        if let Some(env) = &self.env {
            validate_environment(env)?;
        }
        Ok(())
    }
}

/// Explicit configuration patch request with tri-state updates.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceConfigPatch {
    pub java_override: SettingUpdate<String>,
    pub resolution: SettingUpdate<InstanceResolution>,
    pub memory: SettingUpdate<MemoryPolicy>,
    pub jvm_args: SettingUpdate<Vec<String>>,
    pub game_args: SettingUpdate<Vec<String>>,
    pub env: SettingUpdate<BTreeMap<String, String>>,
    pub account_id: SettingUpdate<AccountId>,
}

/// Effective resolved instance configuration after hierarchy evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveInstanceConfig {
    pub java_override: Option<String>,
    pub resolution: Option<InstanceResolution>,
    pub memory: Option<MemoryPolicy>,
    pub jvm_args: Vec<String>,
    pub game_args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub account_id: Option<AccountId>,
}

fn validate_arguments(args: &[String], field: &'static str) -> Result<()> {
    if args.len() > MAX_CONFIG_ARGUMENTS {
        return Err(instance_error(format!(
            "{field} exceeds maximum argument count ({MAX_CONFIG_ARGUMENTS})"
        )));
    }
    for arg in args {
        if arg.len() > MAX_CONFIG_ARGUMENT_BYTES {
            return Err(instance_error(format!(
                "{field} entry exceeds maximum byte length ({MAX_CONFIG_ARGUMENT_BYTES})"
            )));
        }
        if arg.contains('\0') {
            return Err(instance_error(format!(
                "{field} entry contains forbidden NUL character"
            )));
        }
    }
    Ok(())
}

fn validate_environment(env: &BTreeMap<String, String>) -> Result<()> {
    if env.len() > MAX_CONFIG_ENV_ENTRIES {
        return Err(instance_error(format!(
            "environment variables map exceeds maximum count ({MAX_CONFIG_ENV_ENTRIES})"
        )));
    }
    for (k, v) in env {
        if k.is_empty() || k.len() > 256 {
            return Err(instance_error("environment key length is invalid"));
        }
        if v.len() > 4096 {
            return Err(instance_error(
                "environment value length exceeds 4096 bytes",
            ));
        }
        if k.contains('\0') || v.contains('\0') {
            return Err(instance_error(
                "environment variable contains forbidden NUL",
            ));
        }
    }
    Ok(())
}
