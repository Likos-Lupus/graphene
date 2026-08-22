use crate::config::{
    model::{EffectiveInstanceConfig, GlobalInstanceConfig, InstanceConfig, InstanceConfigPatch},
    setting::SettingUpdate,
};
use std::collections::BTreeMap;

/// Evaluates configuration hierarchy and returns the effective resolved configuration.
///
/// Precedence:
/// `built-in defaults < global defaults < per-instance explicit overrides`
#[must_use]
pub fn resolve_effective_config(
    global: Option<&GlobalInstanceConfig>,
    instance: Option<&InstanceConfig>,
) -> EffectiveInstanceConfig {
    let mut java_override = None;
    let mut resolution = None;
    let mut memory = None;
    let mut jvm_args = Vec::new();
    let mut game_args = Vec::new();
    let mut env = BTreeMap::new();
    let mut account_id = None;

    if let Some(g) = global {
        if let Some(java) = &g.java_override {
            java_override = Some(java.clone());
        }
        if let Some(res) = g.resolution {
            resolution = Some(res);
        }
        if let Some(mem) = g.memory {
            memory = Some(mem);
        }
        jvm_args.clone_from(&g.jvm_args);
        game_args.clone_from(&g.game_args);
        env.clone_from(&g.env);
        account_id = g.account_id;
    }

    if let Some(inst) = instance {
        if let Some(java) = &inst.java_override {
            java_override = Some(java.clone());
        }
        if let Some(res) = inst.resolution {
            resolution = Some(res);
        }
        if let Some(mem) = inst.memory {
            memory = Some(mem);
        }
        if let Some(inst_jvm) = &inst.jvm_args {
            jvm_args.clone_from(inst_jvm);
        }
        if let Some(inst_game) = &inst.game_args {
            game_args.clone_from(inst_game);
        }
        if let Some(inst_env) = &inst.env {
            for (k, v) in inst_env {
                env.insert(k.clone(), v.clone());
            }
        }
        if let Some(acc) = inst.account_id {
            account_id = Some(acc);
        }
    }

    EffectiveInstanceConfig {
        java_override,
        resolution,
        memory,
        jvm_args,
        game_args,
        env,
        account_id,
    }
}

/// Applies an explicit tri-state patch to an existing per-instance configuration.
pub fn apply_patch_to_instance(config: &mut InstanceConfig, patch: &InstanceConfigPatch) {
    patch.java_override.apply_to(&mut config.java_override);
    patch.resolution.apply_to(&mut config.resolution);
    patch.memory.apply_to(&mut config.memory);
    patch.jvm_args.apply_to(&mut config.jvm_args);
    patch.game_args.apply_to(&mut config.game_args);
    patch.env.apply_to(&mut config.env);
    patch.account_id.apply_to(&mut config.account_id);
}

/// Applies an explicit tri-state patch to global default instance configuration.
pub fn apply_patch_to_global(config: &mut GlobalInstanceConfig, patch: &InstanceConfigPatch) {
    patch.java_override.apply_to(&mut config.java_override);
    patch.resolution.apply_to(&mut config.resolution);
    patch.memory.apply_to(&mut config.memory);
    match &patch.jvm_args {
        SettingUpdate::Unchanged => {}
        SettingUpdate::Set(args) => config.jvm_args.clone_from(args),
        SettingUpdate::Inherit => config.jvm_args.clear(),
    }
    match &patch.game_args {
        SettingUpdate::Unchanged => {}
        SettingUpdate::Set(args) => config.game_args.clone_from(args),
        SettingUpdate::Inherit => config.game_args.clear(),
    }
    match &patch.env {
        SettingUpdate::Unchanged => {}
        SettingUpdate::Set(env) => config.env.clone_from(env),
        SettingUpdate::Inherit => config.env.clear(),
    }
    patch.account_id.apply_to(&mut config.account_id);
}
