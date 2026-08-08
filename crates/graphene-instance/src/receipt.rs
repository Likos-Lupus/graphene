use crate::{ManagedRelativePath, error::instance_error, path::validate_managed_relative_path};
use graphene_core::{ArtifactIntegrity, ErrorCode, ErrorKind, GrapheneError, InstanceId, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, collections::BTreeSet, fmt};

pub const INSTALL_RECEIPT_SCHEMA_VERSION: u32 = 1;
pub const INSTALL_FORMAT_VERSION: u32 = 1;

/// Durable artifact reference: managed-relative path plus declared integrity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledArtifact {
    pub path: ManagedRelativePath,
    pub integrity: ArtifactIntegrity,
    pub expected_size: Option<u64>,
}

/// Durable library materialization recorded without provider DTOs or absolute host paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledLibrary {
    /// Canonical normalized Maven coordinate string for diagnostics and future migrations.
    pub coordinate: String,
    pub classpath: Option<InstalledArtifact>,
    pub native_archive: Option<InstalledArtifact>,
}

/// Provider-neutral Java requirement persisted with the committed install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledJavaRequirement {
    pub major_version: u32,
    pub component_hint: Option<String>,
}

/// Persisted rule action for launch arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum InstalledRuleAction {
    Allow,
    Disallow,
}

/// Persisted, normalized launch rule. Values are Graphene-normalized names, not provider DTOs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledRule {
    pub action: InstalledRuleAction,
    pub os_name: Option<String>,
    pub os_architecture: Option<String>,
    pub os_version_pattern: Option<String>,
    pub features: BTreeMap<String, bool>,
}

/// Persisted launch argument template. A literal has no rules; a conditional retains its rules and
/// ordered values so launch planning can evaluate runtime-only feature inputs without networking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum InstalledArgument {
    Literal(String),
    Conditional {
        rules: Vec<InstalledRule>,
        values: Vec<String>,
    },
}

/// Provider-neutral durable install receipt at `.graphene/install.json`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallReceipt {
    pub schema_version: u32,
    pub install_format_version: u32,
    pub instance_id: InstanceId,
    pub requested_version: String,
    pub resolved_version: String,
    pub version_type: String,
    pub main_class: String,
    pub java_requirement: InstalledJavaRequirement,
    pub client: InstalledArtifact,
    pub libraries: Vec<InstalledLibrary>,
    pub asset_index_id: String,
    pub asset_index: InstalledArtifact,
    pub assets_root: ManagedRelativePath,
    pub natives_directory: ManagedRelativePath,
    pub logging_configuration: Option<InstalledArtifact>,
    pub logging_argument: Option<String>,
    pub jvm_arguments: Vec<InstalledArgument>,
    pub game_arguments: Vec<InstalledArgument>,
}

impl fmt::Debug for InstallReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstallReceipt")
            .field("schema_version", &self.schema_version)
            .field("install_format_version", &self.install_format_version)
            .field("instance_id", &self.instance_id)
            .field("requested_version", &self.requested_version)
            .field("resolved_version", &self.resolved_version)
            .field("version_type", &self.version_type)
            .field("main_class", &self.main_class)
            .field("java_requirement", &self.java_requirement)
            .field("client", &self.client)
            .field("libraries", &self.libraries)
            .field("asset_index_id", &self.asset_index_id)
            .field("asset_index", &self.asset_index)
            .field("assets_root", &self.assets_root)
            .field("natives_directory", &self.natives_directory)
            .field("logging_configuration", &self.logging_configuration)
            .field("logging_argument", &self.logging_argument)
            .field("jvm_arguments", &self.jvm_arguments)
            // Templates contain placeholder names only. Ephemeral session values never enter the receipt.
            .field("game_arguments", &self.game_arguments)
            .finish()
    }
}

impl InstallReceipt {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != INSTALL_RECEIPT_SCHEMA_VERSION
            || self.install_format_version != INSTALL_FORMAT_VERSION
        {
            return Err(instance_error(
                "install receipt schema or format version is unsupported",
            ));
        }

        validate_identifier(&self.requested_version, 128, "requested Minecraft version")?;
        validate_identifier(&self.resolved_version, 128, "resolved Minecraft version")?;
        validate_identifier(&self.version_type, 64, "Minecraft version type")?;

        if self.main_class.trim().is_empty()
            || self.main_class.len() > 512
            || self.main_class.contains('\0')
        {
            return Err(instance_error("install receipt has an invalid main class"));
        }

        validate_identifier(&self.asset_index_id, 128, "asset index ID")?;
        if self.java_requirement.major_version == 0 || self.java_requirement.major_version > 255 {
            return Err(instance_error(
                "install receipt has an invalid Java requirement",
            ));
        }

        if self
            .java_requirement
            .component_hint
            .as_ref()
            .is_some_and(|value| value.is_empty() || value.len() > 128 || value.contains('\0'))
        {
            return Err(instance_error(
                "install receipt has an invalid Java component hint",
            ));
        }

        validate_artifact(&self.client)?;
        validate_artifact(&self.asset_index)?;

        if let Some(logging) = &self.logging_configuration {
            validate_artifact(logging)?;
        }

        if self
            .logging_argument
            .as_ref()
            .is_some_and(|value| value.len() > 4096 || value.contains('\0'))
        {
            return Err(instance_error(
                "install receipt logging argument is invalid",
            ));
        }

        let mut classpath = BTreeSet::new();
        for library in &self.libraries {
            if library.coordinate.is_empty()
                || library.coordinate.len() > 512
                || library.coordinate.contains('\0')
            {
                return Err(instance_error(
                    "install receipt has an invalid library coordinate",
                ));
            }

            if let Some(path) = &library.classpath {
                validate_artifact(path)?;
                if !classpath.insert(path.path.as_str().to_owned()) {
                    return Err(instance_error(
                        "install receipt contains duplicate classpath entries",
                    ));
                }
            }

            if let Some(path) = &library.native_archive {
                validate_artifact(path)?;
            }
        }

        validate_arguments(&self.jvm_arguments)?;
        validate_arguments(&self.game_arguments)?;

        Ok(())
    }

    pub fn to_pretty_json(&self) -> Result<Vec<u8>> {
        self.validate()?;
        serde_json::to_vec_pretty(self).map_err(|source| {
            GrapheneError::new(
                ErrorCode::InstallValidationFailed,
                ErrorKind::Install,
                "failed to serialize install receipt",
            )
            .with_source(source)
        })
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(instance_error(
                "committed install receipt exceeds the size limit",
            ));
        }

        let receipt: Self = serde_json::from_slice(bytes).map_err(|source| {
            GrapheneError::new(
                ErrorCode::LaunchInstanceInvalid,
                ErrorKind::Launch,
                "failed to parse committed install receipt",
            )
            .with_source(source)
        })?;

        receipt.validate()?;
        Ok(receipt)
    }
}

fn validate_artifact(artifact: &InstalledArtifact) -> Result<()> {
    validate_managed_relative_path(artifact.path.as_str())?;

    if artifact.expected_size == Some(0) {
        return Err(instance_error(
            "install receipt declares a zero-sized required artifact",
        ));
    }

    Ok(())
}

fn validate_arguments(arguments: &[InstalledArgument]) -> Result<()> {
    if arguments.len() > 4096 {
        return Err(instance_error(
            "install receipt contains too many launch arguments",
        ));
    }

    for argument in arguments {
        match argument {
            InstalledArgument::Literal(value) => validate_argument_value(value)?,
            InstalledArgument::Conditional { rules, values } => {
                if rules.len() > 128 || values.is_empty() || values.len() > 128 {
                    return Err(instance_error(
                        "install receipt contains an oversized conditional argument",
                    ));
                }

                for rule in rules {
                    if rule.features.len() > 128 {
                        return Err(instance_error(
                            "install receipt launch rule contains too many features",
                        ));
                    }

                    for name in rule.features.keys() {
                        validate_identifier(name, 128, "launch feature name")?;
                    }
                    for value in [
                        &rule.os_name,
                        &rule.os_architecture,
                        &rule.os_version_pattern,
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if value.is_empty() || value.len() > 256 || value.contains('\0') {
                            return Err(instance_error(
                                "install receipt contains an invalid launch rule",
                            ));
                        }
                    }
                }

                for value in values {
                    validate_argument_value(value)?;
                }
            }
        }
    }

    Ok(())
}

fn validate_argument_value(value: &str) -> Result<()> {
    if value.len() > 16 * 1024 || value.contains('\0') {
        return Err(instance_error(
            "install receipt contains an invalid launch argument template",
        ));
    }

    Ok(())
}

fn validate_identifier(value: &str, maximum: usize, field: &'static str) -> Result<()> {
    if value.trim().is_empty() || value.len() > maximum || value.contains('\0') {
        return Err(instance_error(field));
    }

    Ok(())
}
