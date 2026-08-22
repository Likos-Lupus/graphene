mod merge;
mod model;
mod setting;
#[cfg(test)]
mod tests;

pub use merge::{apply_patch_to_global, apply_patch_to_instance, resolve_effective_config};
pub use model::{
    EffectiveInstanceConfig, GLOBAL_CONFIG_SCHEMA_VERSION, GlobalInstanceConfig,
    INSTANCE_CONFIG_SCHEMA_VERSION, InstanceConfig, InstanceConfigPatch, InstanceResolution,
    MAX_CONFIG_ARGUMENT_BYTES, MAX_CONFIG_ARGUMENTS, MAX_CONFIG_ENV_ENTRIES, MemoryPolicy,
};
pub use setting::SettingUpdate;
