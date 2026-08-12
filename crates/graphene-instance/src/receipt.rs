use crate::{ManagedRelativePath, error::instance_error, path::validate_managed_relative_path};
use graphene_core::{ArtifactIntegrity, ErrorCode, ErrorKind, GrapheneError, InstanceId, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, collections::BTreeSet, fmt};

pub const INSTALL_RECEIPT_SCHEMA_VERSION: u32 = 2;
const LEGACY_INSTALL_RECEIPT_SCHEMA_VERSION: u32 = 1;
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

/// Durable component kind kept independent from provider/domain crate implementation details.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum InstalledComponentKind {
    Minecraft,
    Loader,
    Auxiliary,
}

/// Exact component identity persisted for offline launch and future repair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledComponent {
    pub uid: String,
    pub version: String,
    pub kind: InstalledComponentKind,
    pub provider: String,
    pub provenance: Option<String>,
}

/// Provider-neutral durable install receipt at `.graphene/install.json`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallReceipt {
    pub schema_version: u32,
    pub install_format_version: u32,
    pub instance_id: InstanceId,
    pub requested_version: String,
    pub resolved_version: String,
    #[serde(default)]
    pub components: Vec<InstalledComponent>,
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
            .field("components", &self.components)
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

        if self.components.is_empty() || self.components.len() > 32 {
            return Err(instance_error(
                "install receipt has an invalid component set",
            ));
        }

        let mut component_uids = BTreeSet::new();
        let mut minecraft_components = 0usize;
        let mut loader_components = 0usize;

        for component in &self.components {
            validate_component_uid(&component.uid)?;
            validate_component_version(&component.version)?;
            validate_identifier(&component.provider, 96, "component provider")?;

            if component
                .provenance
                .as_ref()
                .is_some_and(|value| value.len() > 512 || value.chars().any(char::is_control))
            {
                return Err(instance_error(
                    "install receipt component provenance is invalid",
                ));
            }

            if !component_uids.insert(component.uid.as_str()) {
                return Err(instance_error(
                    "install receipt contains duplicate component UIDs",
                ));
            }

            match component.kind {
                InstalledComponentKind::Minecraft => minecraft_components += 1,
                InstalledComponentKind::Loader => loader_components += 1,
                InstalledComponentKind::Auxiliary => {}
            }
        }

        if minecraft_components != 1 || loader_components > 1 {
            return Err(instance_error(
                "install receipt component kinds are invalid",
            ));
        }

        let base = self
            .components
            .iter()
            .find(|component| matches!(component.kind, InstalledComponentKind::Minecraft))
            .expect("count checked");
        if base.uid != "net.minecraft" || base.version != self.resolved_version {
            return Err(instance_error(
                "install receipt base component does not match resolved Minecraft",
            ));
        }

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

        let mut receipt: Self = serde_json::from_slice(bytes).map_err(|source| {
            GrapheneError::new(
                ErrorCode::LaunchInstanceInvalid,
                ErrorKind::Launch,
                "failed to parse committed install receipt",
            )
            .with_source(source)
        })?;

        if receipt.schema_version == LEGACY_INSTALL_RECEIPT_SCHEMA_VERSION {
            if !receipt.components.is_empty() {
                return Err(instance_error(
                    "legacy install receipt unexpectedly contains components",
                ));
            }

            receipt.components = vec![InstalledComponent {
                uid: "net.minecraft".to_owned(),
                version: receipt.resolved_version.clone(),
                kind: InstalledComponentKind::Minecraft,
                provider: "mojang".to_owned(),
                provenance: Some("implicit-phase1-receipt".to_owned()),
            }];
            receipt.schema_version = INSTALL_RECEIPT_SCHEMA_VERSION;
        }

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

fn validate_component_uid(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || value.starts_with('.')
        || value.ends_with('.')
        || value.contains("..")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(instance_error("install receipt component UID is invalid"));
    }

    Ok(())
}

fn validate_component_version(value: &str) -> Result<()> {
    if value.trim().is_empty()
        || value.len() > 192
        || value.chars().any(char::is_control)
        || value.contains('/')
        || value.contains('\\')
    {
        return Err(instance_error(
            "install receipt component version is invalid",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(path: &str) -> InstalledArtifact {
        InstalledArtifact {
            path: ManagedRelativePath::new(path).expect("managed path"),
            integrity: ArtifactIntegrity::none().with_sha1(
                "0123456789abcdef0123456789abcdef01234567"
                    .parse()
                    .expect("sha1"),
            ),
            expected_size: Some(1),
        }
    }

    fn current_receipt() -> InstallReceipt {
        InstallReceipt {
            schema_version: INSTALL_RECEIPT_SCHEMA_VERSION,
            install_format_version: INSTALL_FORMAT_VERSION,
            instance_id: InstanceId::new(),
            requested_version: "1.21.1".to_owned(),
            resolved_version: "1.21.1".to_owned(),
            components: vec![InstalledComponent {
                uid: "net.minecraft".to_owned(),
                version: "1.21.1".to_owned(),
                kind: InstalledComponentKind::Minecraft,
                provider: "mojang".to_owned(),
                provenance: Some("version-manifest".to_owned()),
            }],
            version_type: "release".to_owned(),
            main_class: "net.minecraft.client.main.Main".to_owned(),
            java_requirement: InstalledJavaRequirement {
                major_version: 21,
                component_hint: Some("java-runtime-delta".to_owned()),
            },
            client: artifact("shared/versions/1.21.1/client.jar"),
            libraries: Vec::new(),
            asset_index_id: "17".to_owned(),
            asset_index: artifact("shared/assets/indexes/17.json"),
            assets_root: ManagedRelativePath::new("shared/assets").expect("assets"),
            natives_directory: ManagedRelativePath::new("natives").expect("natives"),
            logging_configuration: None,
            logging_argument: None,
            jvm_arguments: Vec::new(),
            game_arguments: Vec::new(),
        }
    }

    #[test]
    fn phase1_receipt_is_migrated_in_memory_to_an_implicit_minecraft_component() {
        let mut value = serde_json::to_value(current_receipt()).expect("serialize");
        let object = value.as_object_mut().expect("receipt object");
        object.insert("schema_version".to_owned(), serde_json::json!(1));
        object.remove("components");
        let bytes = serde_json::to_vec(&value).expect("json");

        let migrated = InstallReceipt::from_json(&bytes).expect("legacy receipt");
        assert_eq!(migrated.schema_version, INSTALL_RECEIPT_SCHEMA_VERSION);
        assert_eq!(migrated.components.len(), 1);
        assert_eq!(migrated.components[0].uid, "net.minecraft");
        assert_eq!(migrated.components[0].version, "1.21.1");
        assert_eq!(migrated.components[0].provider, "mojang");
    }

    #[test]
    fn malformed_legacy_receipt_with_components_is_rejected() {
        let mut receipt = current_receipt();
        receipt.schema_version = 1;
        let bytes = serde_json::to_vec(&receipt).expect("json");
        assert!(InstallReceipt::from_json(&bytes).is_err());
    }

    #[test]
    fn loader_receipt_requires_exactly_one_base_and_at_most_one_primary_loader() {
        let mut receipt = current_receipt();

        receipt.components.push(InstalledComponent {
            uid: "net.fabricmc.fabric-loader".to_owned(),
            version: "0.16.10".to_owned(),
            kind: InstalledComponentKind::Loader,
            provider: "fabric-meta".to_owned(),
            provenance: Some("profile".to_owned()),
        });

        receipt.validate().expect("one loader");

        receipt.components.push(InstalledComponent {
            uid: "net.minecraftforge.forge".to_owned(),
            version: "52.0.1".to_owned(),
            kind: InstalledComponentKind::Loader,
            provider: "forge-maven".to_owned(),
            provenance: None,
        });

        assert!(receipt.validate().is_err());
    }
}
