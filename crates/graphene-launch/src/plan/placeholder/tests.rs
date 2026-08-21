use super::*;
use crate::{LaunchResolution, LaunchSession};
use graphene_core::{ErrorCode, SensitiveString};
use std::collections::BTreeMap;

fn values(resolution: Option<LaunchResolution>) -> PlaceholderValues {
    let session = LaunchSession {
        username: "Fixture Player".into(),
        uuid: "00000000-0000-0000-0000-000000000001".into(),
        access_token: SensitiveString::new("secret"),
        user_type: "msa".into(),
        client_id: Some(SensitiveString::new("client-secret")),
        xuid: Some(SensitiveString::new("xuid-secret")),
    };
    PlaceholderValues {
        username: session.username,
        uuid: session.uuid,
        access_token: session.access_token,
        user_type: session.user_type,
        client_id: session.client_id,
        xuid: session.xuid,
        version_name: "fixture".into(),
        version_type: "release".into(),
        game_directory: "/game".into(),
        assets_root: "/assets".into(),
        asset_index_name: "fixture-assets".into(),
        natives_directory: "/natives".into(),
        library_directory: "/libraries".into(),
        resolution,
    }
}

#[test]
fn placeholder_expansion_classifies_secrets_and_missing_values() {
    let values = values(None);
    assert!(matches!(
        expand_argument("${auth_access_token}", &values).expect("token"),
        LaunchArgument::Secret(_)
    ));
    assert!(matches!(
        expand_argument("${classpath}", &values).expect("classpath"),
        LaunchArgument::Classpath
    ));
    assert_eq!(
        expand_argument("--name=${auth_player_name}", &values).expect("name"),
        LaunchArgument::Plain("--name=Fixture Player".into())
    );
    assert_eq!(
        expand_argument("${resolution_width}", &values)
            .expect_err("missing")
            .code,
        ErrorCode::LaunchPlaceholderMissing
    );
}

#[test]
fn filtered_argument_does_not_require_missing_placeholder() {
    let argument = InstalledArgument::Conditional {
        rules: vec![InstalledRule {
            action: InstalledRuleAction::Allow,
            os_name: None,
            os_architecture: None,
            os_version_pattern: None,
            features: BTreeMap::from([("has_custom_resolution".into(), true)]),
        }],
        values: vec!["${resolution_width}".into()],
    };
    let context = RuleContext {
        os: MinecraftOs::Linux,
        arch: MinecraftArch::X86_64,
        os_version: None,
        features: BTreeMap::new(),
    };
    assert!(
        resolve_arguments(&[argument], &context, &values(None))
            .expect("filtered")
            .is_empty()
    );
}

#[test]
fn argument_resolution_preserves_jvm_and_game_order() {
    let context = RuleContext {
        os: MinecraftOs::Linux,
        arch: MinecraftArch::X86_64,
        os_version: None,
        features: BTreeMap::new(),
    };
    let arguments = vec![
        InstalledArgument::Literal("first".into()),
        InstalledArgument::Literal("--name=${auth_player_name}".into()),
        InstalledArgument::Literal("third".into()),
    ];
    assert_eq!(
        resolve_arguments(
            &arguments,
            &context,
            &values(Some(LaunchResolution {
                width: 800,
                height: 600,
            })),
        )
        .expect("arguments"),
        vec![
            LaunchArgument::Plain("first".into()),
            LaunchArgument::Plain("--name=Fixture Player".into()),
            LaunchArgument::Plain("third".into()),
        ]
    );
}

#[test]
fn composite_secret_placeholder_is_rejected_instead_of_declassified() {
    assert_eq!(
        expand_argument("--token=${auth_access_token}", &values(None))
            .expect_err("secret must stay classified")
            .code,
        ErrorCode::LaunchPlanInvalid
    );
}
