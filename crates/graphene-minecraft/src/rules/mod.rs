mod pattern;

use graphene_core::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use pattern::safe_pattern_matches;

/// Minecraft rule action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RuleAction {
    Allow,
    Disallow,
}

/// Minecraft-facing OS identity; intentionally independent of `graphene-platform`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MinecraftOs {
    Windows,
    Linux,
    Osx,
    Other(String),
}

/// Minecraft-facing architecture identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MinecraftArch {
    X86,
    X86_64,
    AArch64,
    Other(String),
}

impl MinecraftArch {
    #[must_use]
    pub fn classifier_value(&self) -> &str {
        match self {
            Self::X86 => "32",
            Self::X86_64 => "64",
            Self::AArch64 => "arm64",
            Self::Other(value) => value,
        }
    }
}

/// Optional OS constraints attached to a Mojang rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OsRule {
    pub name: Option<MinecraftOs>,
    pub architecture: Option<MinecraftArch>,
    pub version_pattern: Option<String>,
}

/// Normalized Mojang rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub action: RuleAction,
    pub os: Option<OsRule>,
    pub features: BTreeMap<String, bool>,
}

/// Deterministic rule-evaluation inputs supplied by the service/platform boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleContext {
    pub os: MinecraftOs,
    pub arch: MinecraftArch,
    pub os_version: Option<String>,
    pub features: BTreeMap<String, bool>,
}

impl Rule {
    fn matches(&self, context: &RuleContext) -> Result<bool> {
        if let Some(os) = &self.os {
            if os.name.as_ref().is_some_and(|name| name != &context.os) {
                return Ok(false);
            }

            if os
                .architecture
                .as_ref()
                .is_some_and(|arch| arch != &context.arch)
            {
                return Ok(false);
            }

            if let Some(pattern) = &os.version_pattern {
                let Some(version) = &context.os_version else {
                    return Ok(false);
                };

                if !safe_pattern_matches(pattern, version)? {
                    return Ok(false);
                }
            }
        }

        Ok(self.features.iter().all(|(name, required)| {
            context.features.get(name).copied().unwrap_or(false) == *required
        }))
    }
}

/// Applies Mojang's last-matching-rule semantics.
pub fn rules_allow(rules: &[Rule], context: &RuleContext) -> Result<bool> {
    if rules.is_empty() {
        return Ok(true);
    }

    let mut allowed = false;
    for rule in rules {
        if rule.matches(context)? {
            allowed = matches!(rule.action, RuleAction::Allow);
        }
    }

    Ok(allowed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::context;

    #[test]
    fn no_rules_are_allowed_and_last_matching_action_wins() {
        let ctx = context(MinecraftOs::Windows, MinecraftArch::X86_64);
        assert!(rules_allow(&[], &ctx).expect("rules"));
        let allow_windows = vec![Rule {
            action: RuleAction::Allow,
            os: Some(OsRule {
                name: Some(MinecraftOs::Windows),
                ..OsRule::default()
            }),
            features: BTreeMap::new(),
        }];
        assert!(rules_allow(&allow_windows, &ctx).expect("allow"));
        assert!(
            !rules_allow(
                &allow_windows,
                &context(MinecraftOs::Linux, MinecraftArch::X86_64)
            )
            .expect("non-match")
        );
        let rules = vec![
            Rule {
                action: RuleAction::Allow,
                os: None,
                features: BTreeMap::new(),
            },
            Rule {
                action: RuleAction::Disallow,
                os: Some(OsRule {
                    name: Some(MinecraftOs::Windows),
                    ..OsRule::default()
                }),
                features: BTreeMap::new(),
            },
        ];
        assert!(!rules_allow(&rules, &ctx).expect("rules"));
    }

    #[test]
    fn os_arch_features_and_patterns_are_deterministic() {
        let mut ctx = context(MinecraftOs::Windows, MinecraftArch::X86_64);
        ctx.features.insert("has_custom_resolution".into(), true);
        let rule = Rule {
            action: RuleAction::Allow,
            os: Some(OsRule {
                name: Some(MinecraftOs::Windows),
                architecture: Some(MinecraftArch::X86_64),
                version_pattern: Some(r"^10\..*".into()),
            }),
            features: BTreeMap::from([("has_custom_resolution".into(), true)]),
        };
        assert!(rule.matches(&ctx).expect("match"));
        let mut wrong_arch = ctx.clone();
        wrong_arch.arch = MinecraftArch::AArch64;
        assert!(!rule.matches(&wrong_arch).expect("architecture non-match"));
        let mut missing_feature = ctx.clone();
        missing_feature
            .features
            .insert("has_custom_resolution".into(), false);
        assert!(!rule.matches(&missing_feature).expect("feature non-match"));
        assert!(safe_pattern_matches(r"^10\..*", "10.0").expect("pattern"));
        assert_eq!(
            safe_pattern_matches("[broken", "10")
                .expect_err("invalid")
                .code,
            graphene_core::ErrorCode::MinecraftRuleInvalid
        );
    }
}
