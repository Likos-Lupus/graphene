use super::LaunchArgument;
use crate::{
    error::launch_error,
    request::{LaunchResolution, validate_argument},
};
use graphene_core::{ErrorCode, GrapheneError, Result, SensitiveString};
use graphene_instance::{InstalledArgument, InstalledRule, InstalledRuleAction};
use graphene_minecraft::{
    MinecraftArch, MinecraftOs, OsRule, Rule, RuleAction, RuleContext, rules_allow,
};
use std::collections::BTreeMap;

pub(super) struct PlaceholderValues {
    pub(super) username: String,
    pub(super) uuid: String,
    pub(super) access_token: SensitiveString,
    pub(super) user_type: String,
    pub(super) client_id: Option<SensitiveString>,
    pub(super) xuid: Option<SensitiveString>,
    pub(super) version_name: String,
    pub(super) version_type: String,
    pub(super) game_directory: String,
    pub(super) assets_root: String,
    pub(super) asset_index_name: String,
    pub(super) natives_directory: String,
    pub(super) library_directory: String,
    pub(super) resolution: Option<LaunchResolution>,
}

pub(super) fn resolve_arguments(
    arguments: &[InstalledArgument],
    context: &RuleContext,
    values: &PlaceholderValues,
) -> Result<Vec<LaunchArgument>> {
    let mut result = Vec::new();
    for argument in arguments {
        match argument {
            InstalledArgument::Literal(value) => result.push(expand_argument(value, values)?),
            InstalledArgument::Conditional {
                rules,
                values: argument_values,
            } => {
                let rules = rules.iter().map(restore_rule).collect::<Result<Vec<_>>>()?;
                if rules_allow(&rules, context)? {
                    for value in argument_values {
                        result.push(expand_argument(value, values)?);
                    }
                }
            }
            _ => {
                return Err(launch_error(
                    ErrorCode::LaunchPlanInvalid,
                    "installed argument variant is unsupported",
                ));
            }
        }
    }

    Ok(result)
}

fn restore_rule(value: &InstalledRule) -> Result<Rule> {
    let os = if value.os_name.is_some()
        || value.os_architecture.is_some()
        || value.os_version_pattern.is_some()
    {
        Some(OsRule {
            name: value.os_name.as_deref().map(restore_os),
            architecture: value.os_architecture.as_deref().map(restore_arch),
            version_pattern: value.os_version_pattern.clone(),
        })
    } else {
        None
    };

    Ok(Rule {
        action: match value.action {
            InstalledRuleAction::Allow => RuleAction::Allow,
            InstalledRuleAction::Disallow => RuleAction::Disallow,
            _ => {
                return Err(launch_error(
                    ErrorCode::LaunchPlanInvalid,
                    "installed rule action is unsupported",
                ));
            }
        },
        os,
        features: value.features.clone(),
    })
}

fn restore_os(value: &str) -> MinecraftOs {
    match value {
        "windows" => MinecraftOs::Windows,
        "linux" => MinecraftOs::Linux,
        "osx" => MinecraftOs::Osx,
        other => MinecraftOs::Other(other.to_owned()),
    }
}

fn restore_arch(value: &str) -> MinecraftArch {
    match value {
        "x86" => MinecraftArch::X86,
        "x86_64" => MinecraftArch::X86_64,
        "aarch64" => MinecraftArch::AArch64,
        other => MinecraftArch::Other(other.to_owned()),
    }
}

pub(super) fn expand_argument(
    template: &str,
    values: &PlaceholderValues,
) -> Result<LaunchArgument> {
    validate_argument(template)?;

    if template == "${auth_access_token}" {
        return Ok(LaunchArgument::Secret(values.access_token.clone()));
    }

    if template == "${clientid}" {
        return values
            .client_id
            .clone()
            .map(LaunchArgument::Secret)
            .ok_or_else(|| missing_placeholder("clientid"));
    }

    if template == "${auth_xuid}" {
        return values
            .xuid
            .clone()
            .map(LaunchArgument::Secret)
            .ok_or_else(|| missing_placeholder("auth_xuid"));
    }

    if template == "${classpath}" {
        return Ok(LaunchArgument::Classpath);
    }

    if template == "${classpath_separator}" {
        return Ok(LaunchArgument::ClasspathSeparator);
    }

    for secret in ["auth_access_token", "clientid", "auth_xuid"] {
        if template.contains(&format!("${{{secret}}}")) {
            return Err(launch_error(
                ErrorCode::LaunchPlanInvalid,
                "secret placeholder must occupy a complete argv element",
            )
            .with_context("placeholder", secret));
        }
    }

    if template.contains("${classpath}") || template.contains("${classpath_separator}") {
        return Err(launch_error(
            ErrorCode::LaunchPlanInvalid,
            "classpath placeholder must occupy a complete argv element",
        ));
    }

    let mut replacements = BTreeMap::<&str, Option<String>>::new();
    replacements.insert("auth_player_name", Some(values.username.clone()));
    replacements.insert("version_name", Some(values.version_name.clone()));
    replacements.insert("game_directory", Some(values.game_directory.clone()));
    replacements.insert("assets_root", Some(values.assets_root.clone()));
    replacements.insert("game_assets", Some(values.assets_root.clone()));
    replacements.insert("assets_index_name", Some(values.asset_index_name.clone()));
    replacements.insert("auth_uuid", Some(values.uuid.clone()));
    replacements.insert("user_type", Some(values.user_type.clone()));
    replacements.insert("version_type", Some(values.version_type.clone()));
    replacements.insert("natives_directory", Some(values.natives_directory.clone()));
    replacements.insert("launcher_name", Some("graphene".to_owned()));
    replacements.insert(
        "launcher_version",
        Some(env!("CARGO_PKG_VERSION").to_owned()),
    );
    replacements.insert("library_directory", Some(values.library_directory.clone()));
    replacements.insert("user_properties", Some("{}".to_owned()));
    replacements.insert("auth_session", Some("-".to_owned()));
    replacements.insert(
        "resolution_width",
        values
            .resolution
            .map(|resolution| resolution.width.to_string()),
    );
    replacements.insert(
        "resolution_height",
        values
            .resolution
            .map(|resolution| resolution.height.to_string()),
    );

    let expanded = substitute_plain(template, &replacements)?;
    validate_argument(&expanded)?;
    Ok(LaunchArgument::Plain(expanded))
}

pub(super) fn expand_logging_argument(template: &str, path: &str) -> Result<LaunchArgument> {
    let mut replacements = BTreeMap::new();
    replacements.insert("path", Some(path.to_owned()));
    let value = substitute_plain(template, &replacements)?;

    validate_argument(&value)?;
    Ok(LaunchArgument::Plain(value))
}

fn substitute_plain(
    template: &str,
    replacements: &BTreeMap<&str, Option<String>>,
) -> Result<String> {
    let mut output = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(start) = rest.find("${") {
        output.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find('}').ok_or_else(|| {
            launch_error(
                ErrorCode::LaunchPlaceholderMissing,
                "launch placeholder is malformed",
            )
        })?;

        let name = &after[..end];
        if name.is_empty() || name.len() > 128 {
            return Err(launch_error(
                ErrorCode::LaunchPlaceholderMissing,
                "launch placeholder name is invalid",
            ));
        }

        let replacement = replacements
            .get(name)
            .ok_or_else(|| missing_placeholder(name))?;
        let replacement = replacement
            .as_ref()
            .ok_or_else(|| missing_placeholder(name))?;
        output.push_str(replacement);
        rest = &after[end + 1..];
    }
    output.push_str(rest);

    Ok(output)
}

fn missing_placeholder(name: &str) -> GrapheneError {
    launch_error(
        ErrorCode::LaunchPlaceholderMissing,
        "required launch placeholder value is unavailable",
    )
    .with_context("placeholder", name)
}

#[cfg(test)]
mod tests {
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
}
