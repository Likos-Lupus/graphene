mod build;
mod path;
mod placeholder;
mod redact;

use crate::error::launch_error;
use graphene_core::{ErrorCode, InstanceId, Result, SensitiveString};
use graphene_java::JavaRuntime;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

use self::path::{validate_directory, validate_ordinary_file};

/// Secret classification is retained until the direct process boundary. Classpath expansion is
/// deferred so the platform separator is applied only when argv is materialized.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LaunchArgument {
    Plain(String),
    Secret(SensitiveString),
    Classpath,
    ClasspathSeparator,
}

impl std::fmt::Debug for LaunchArgument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plain(value) => f.debug_tuple("Plain").field(value).finish(),
            Self::Secret(_) => f.write_str("Secret(<redacted>)"),
            Self::Classpath => f.write_str("Classpath(<deferred>)"),
            Self::ClasspathSeparator => f.write_str("ClasspathSeparator(<deferred>)"),
        }
    }
}

impl LaunchArgument {
    fn redacted(&self) -> String {
        match self {
            Self::Plain(value) => value.clone(),
            Self::Secret(_) => "<secret>".to_owned(),
            Self::Classpath => "<classpath>".to_owned(),
            Self::ClasspathSeparator => "<classpath-separator>".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentDelta {
    pub values: BTreeMap<String, String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub instance_id: InstanceId,
    pub java: JavaRuntime,
    pub working_directory: PathBuf,
    pub environment: EnvironmentDelta,
    pub jvm_args: Vec<LaunchArgument>,
    pub classpath: Vec<PathBuf>,
    pub classpath_separator: char,
    pub main_class: String,
    pub game_args: Vec<LaunchArgument>,
    pub natives_directory: PathBuf,
}

impl std::fmt::Debug for LaunchPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LaunchPlan")
            .field("instance_id", &self.instance_id)
            .field("java", &self.java)
            .field("working_directory", &self.working_directory)
            .field(
                "environment_keys",
                &self.environment.values.keys().collect::<Vec<_>>(),
            )
            .field(
                "jvm_args",
                &self
                    .jvm_args
                    .iter()
                    .map(LaunchArgument::redacted)
                    .collect::<Vec<_>>(),
            )
            .field("classpath", &self.classpath)
            .field("classpath_separator", &self.classpath_separator)
            .field("main_class", &self.main_class)
            .field(
                "game_args",
                &self
                    .game_args
                    .iter()
                    .map(LaunchArgument::redacted)
                    .collect::<Vec<_>>(),
            )
            .field("natives_directory", &self.natives_directory)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactedLaunchPlan {
    pub instance_id: String,
    pub java: String,
    pub working_directory: String,
    pub environment_keys: Vec<String>,
    pub jvm_args: Vec<String>,
    pub classpath: Vec<String>,
    pub classpath_separator: String,
    pub main_class: String,
    pub game_args: Vec<String>,
    pub natives_directory: String,
}

impl LaunchPlan {
    pub fn validate(&self) -> Result<()> {
        validate_ordinary_file(
            &self.java.executable,
            ErrorCode::LaunchPlanInvalid,
            "Java executable is unavailable",
        )?;
        validate_directory(&self.working_directory, "working directory")?;
        validate_directory(&self.natives_directory, "native directory")?;

        if self.main_class.trim().is_empty()
            || self.main_class.len() > 512
            || self.main_class.contains('\0')
        {
            return Err(launch_error(
                ErrorCode::LaunchPlanInvalid,
                "launch main class is invalid",
            ));
        }

        if self.classpath.is_empty() {
            return Err(launch_error(
                ErrorCode::LaunchPlanInvalid,
                "launch classpath is empty",
            ));
        }

        for path in &self.classpath {
            validate_ordinary_file(
                path,
                ErrorCode::LaunchPlanInvalid,
                "classpath entry is unavailable",
            )?;
        }

        if self
            .jvm_args
            .iter()
            .chain(&self.game_args)
            .any(|argument| match argument {
                LaunchArgument::Plain(value) => value.contains("${"),
                _ => false,
            })
        {
            return Err(launch_error(
                ErrorCode::LaunchPlanInvalid,
                "launch plan contains an unresolved placeholder",
            ));
        }

        Ok(())
    }
}

pub use build::plan_from_committed;
