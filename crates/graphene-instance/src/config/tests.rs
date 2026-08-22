use super::*;
use graphene_core::AccountId;
use std::collections::BTreeMap;

#[test]
fn config_precedence_and_merge_rules() {
    let mut global_env = BTreeMap::new();
    global_env.insert("GLOBAL_VAR".to_string(), "1".to_string());
    let global = GlobalInstanceConfig {
        java_override: Some("/usr/bin/java".to_string()),
        resolution: Some(InstanceResolution {
            width: 1280,
            height: 720,
        }),
        jvm_args: vec!["-Xmx4G".to_string()],
        env: global_env,
        ..Default::default()
    };

    let mut inst_env = BTreeMap::new();
    inst_env.insert("INST_VAR".to_string(), "2".to_string());
    let instance = InstanceConfig {
        resolution: Some(InstanceResolution {
            width: 1920,
            height: 1080,
        }),
        env: Some(inst_env),
        ..Default::default()
    };

    let effective = resolve_effective_config(Some(&global), Some(&instance));

    // Java override inherits from global
    assert_eq!(effective.java_override.as_deref(), Some("/usr/bin/java"));
    // Resolution is overridden by instance
    assert_eq!(
        effective.resolution,
        Some(InstanceResolution {
            width: 1920,
            height: 1080
        })
    );
    // JVM args inherit from global
    assert_eq!(effective.jvm_args, vec!["-Xmx4G"]);
    // Env maps merge deterministically
    assert_eq!(
        effective.env.get("GLOBAL_VAR").map(String::as_str),
        Some("1")
    );
    assert_eq!(effective.env.get("INST_VAR").map(String::as_str), Some("2"));
}

#[test]
fn patch_semantics_and_reset_to_inherit() {
    let mut instance = InstanceConfig::default();
    let patch = InstanceConfigPatch {
        resolution: SettingUpdate::Set(InstanceResolution {
            width: 800,
            height: 600,
        }),
        jvm_args: SettingUpdate::Set(vec!["-XX:+UseG1GC".to_string()]),
        ..Default::default()
    };

    apply_patch_to_instance(&mut instance, &patch);
    assert_eq!(
        instance.resolution,
        Some(InstanceResolution {
            width: 800,
            height: 600
        })
    );
    assert_eq!(instance.jvm_args, Some(vec!["-XX:+UseG1GC".to_string()]));

    // Reset resolution to inherit
    let patch2 = InstanceConfigPatch {
        resolution: SettingUpdate::Inherit,
        ..Default::default()
    };

    apply_patch_to_instance(&mut instance, &patch2);
    assert_eq!(instance.resolution, None);
    // jvm_args was unchanged
    assert_eq!(instance.jvm_args, Some(vec!["-XX:+UseG1GC".to_string()]));
}

#[test]
fn account_id_override_and_patch() {
    let acc_id = AccountId::new();
    let mut instance = InstanceConfig::default();
    let patch = InstanceConfigPatch {
        account_id: SettingUpdate::Set(acc_id),
        ..Default::default()
    };

    apply_patch_to_instance(&mut instance, &patch);
    assert_eq!(instance.account_id, Some(acc_id));

    let effective = resolve_effective_config(None, Some(&instance));
    assert_eq!(effective.account_id, Some(acc_id));
}
