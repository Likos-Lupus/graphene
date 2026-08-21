use super::*;

pub(super) fn normalize_version_arguments(
    version: &VersionDto,
) -> Result<(Vec<Argument>, Vec<Argument>)> {
    let mut game = normalize_arguments(version.arguments.game.clone())?;

    if let Some(legacy) = &version.minecraft_arguments {
        game.extend(
            tokenize_legacy_arguments(legacy)?
                .into_iter()
                .map(Argument::Literal),
        );
    }

    Ok((normalize_arguments(version.arguments.jvm.clone())?, game))
}

fn normalize_arguments(values: Vec<ArgumentDto>) -> Result<Vec<Argument>> {
    if values.len() > 4096 {
        return Err(loader_error(
            ErrorCode::LoaderProfileInvalid,
            "loader version contains too many arguments",
        ));
    }

    values
        .into_iter()
        .map(|value| match value {
            ArgumentDto::Literal(value) => {
                validate_text(&value, 16 * 1024, "loader argument is invalid")?;
                Ok(Argument::Literal(value))
            }
            ArgumentDto::Conditional { rules, value } => {
                let values = match value {
                    ArgumentValueDto::One(value) => vec![value],
                    ArgumentValueDto::Many(values) => values,
                };
                for value in &values {
                    validate_text(value, 16 * 1024, "loader conditional argument is invalid")?;
                }
                Ok(Argument::Conditional {
                    rules: rules
                        .iter()
                        .map(normalize_rule)
                        .collect::<Result<Vec<_>>>()?,
                    values,
                })
            }
        })
        .collect()
}

pub(super) fn normalize_rule(value: &RuleDto) -> Result<Rule> {
    let action = match value.action.as_str() {
        "allow" => RuleAction::Allow,
        "disallow" => RuleAction::Disallow,
        _ => {
            return Err(loader_error(
                ErrorCode::LoaderProfileInvalid,
                "loader rule action is unsupported",
            ));
        }
    };

    let os = value.os.as_ref().map(normalize_os_rule).transpose()?;

    Ok(Rule {
        action,
        os,
        features: value.features.clone(),
    })
}

fn normalize_os_rule(value: &OsRuleDto) -> Result<OsRule> {
    let name = value
        .name
        .as_deref()
        .map(|value| match value {
            "windows" => Ok(MinecraftOs::Windows),
            "linux" => Ok(MinecraftOs::Linux),
            "osx" => Ok(MinecraftOs::Osx),
            other if !other.is_empty() && other.len() <= 64 => {
                Ok(MinecraftOs::Other(other.to_owned()))
            }
            _ => Err(loader_error(
                ErrorCode::LoaderProfileInvalid,
                "loader OS rule is invalid",
            )),
        })
        .transpose()?;

    let architecture = value
        .arch
        .as_deref()
        .map(|value| match value {
            "x86" | "32" => Ok(MinecraftArch::X86),
            "x86_64" | "amd64" | "64" => Ok(MinecraftArch::X86_64),
            "aarch64" | "arm64" => Ok(MinecraftArch::AArch64),
            other if !other.is_empty() && other.len() <= 64 => {
                Ok(MinecraftArch::Other(other.to_owned()))
            }
            _ => Err(loader_error(
                ErrorCode::LoaderProfileInvalid,
                "loader architecture rule is invalid",
            )),
        })
        .transpose()?;

    if value
        .version
        .as_ref()
        .is_some_and(|value| value.len() > 256 || value.contains('\0'))
    {
        return Err(loader_error(
            ErrorCode::LoaderProfileInvalid,
            "loader OS version rule exceeds its bound",
        ));
    }

    Ok(OsRule {
        name,
        architecture,
        version_pattern: value.version.clone(),
    })
}
