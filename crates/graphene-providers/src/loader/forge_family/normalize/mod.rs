use super::{
    dto::{
        ArgumentDto, ArgumentValueDto, DataFileDto, DownloadDto, InstallProfileDto, LibraryDto,
        OsRuleDto, ProcessorDto, RuleDto, VersionDto,
    },
    placeholder::parse_argument,
};
use crate::loader::common::{
    MAX_PROFILE_LIBRARIES, artifact, jar, join_base, loader_error, validate_endpoint,
};
use graphene_core::{
    Artifact, ArtifactId, ArtifactIntegrity, ArtifactKind, ErrorCode, OperationController, Result,
};
use graphene_minecraft::{
    Argument, ComponentPreparationRecipe, GeneratedOutput, GeneratedOutputScope, Library,
    LoaderKind, LoaderSupport, ManagedPath, MavenCoordinate, MinecraftArch,
    MinecraftJavaRequirement, MinecraftOs, OsRule, PreparationDataValue, ProcessorSideCondition,
    ProcessorStep, ResolvedArtifact, ResolvedLoader, Rule, RuleAction, tokenize_legacy_arguments,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    path::Path,
};

const INSTALL_PROFILE: &str = "install_profile.json";
const VERSION_JSON: &str = "version.json";
const MAX_PROCESSORS: usize = 128;
const MAX_PROCESSOR_CLASSPATH: usize = 1024;
const MAX_PROCESSOR_OUTPUTS: usize = 512;
const DEFAULT_PROCESSOR_TIMEOUT_SECONDS: u64 = 300;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ForgeFamily {
    Forge,
    NeoForge,
}

impl ForgeFamily {
    fn kind(self) -> LoaderKind {
        match self {
            Self::Forge => LoaderKind::Forge,
            Self::NeoForge => LoaderKind::NeoForge,
        }
    }
}

mod data;
mod embedded;
mod library;
mod processor;
mod validation;
mod version;

#[cfg(test)]
mod tests;

use data::{data_client_raw, normalize_data, output_digest};
use embedded::collect_embedded_inputs;
use library::{CoordinateKey, normalize_remote_library, normalize_version_libraries};
use processor::{NormalizedProcessors, collect_processor_inputs, normalize_processors};
use validation::{validate_key, validate_text};
use version::{normalize_rule, normalize_version_arguments};

pub(crate) fn normalize_verified_installer(
    mut resolved: ResolvedLoader,
    installer_path: &Path,
    operation: &OperationController,
    family: ForgeFamily,
    default_maven_base: &str,
    allow_http: bool,
) -> Result<ResolvedLoader> {
    if resolved.kind != family.kind() {
        return Err(loader_error(
            ErrorCode::LoaderInstallerInvalid,
            "loader installer family does not match the resolved loader",
        ));
    }

    validate_endpoint(default_maven_base, allow_http)?;
    let cancellation = operation.cancellation_token();
    let names = jar::list_entry_names(installer_path, &cancellation)?;
    let name_set = names.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let selected = jar::read_selected_entries(
        installer_path,
        &[INSTALL_PROFILE, VERSION_JSON],
        &cancellation,
    )?;
    let profile_bytes = selected.get(INSTALL_PROFILE).ok_or_else(|| {
        loader_error(
            ErrorCode::LoaderInstallerSpecUnsupported,
            "loader installer does not contain a supported install profile",
        )
    })?;

    if profile_bytes.len() > crate::loader::common::MAX_LOADER_METADATA_BYTES {
        return Err(loader_error(
            ErrorCode::LoaderInstallerInvalid,
            "loader install profile exceeds its metadata size bound",
        ));
    }

    let profile: InstallProfileDto = serde_json::from_slice(profile_bytes).map_err(|source| {
        loader_error(
            ErrorCode::LoaderInstallerInvalid,
            "loader install profile is invalid JSON",
        )
        .with_source(source)
    })?;

    let modern = profile.spec.is_some() || !profile.processors.is_empty() || profile.json.is_some();
    if !modern {
        resolved.support = LoaderSupport::MetadataOnly {
            reason: "legacy launcher-profile installer family is metadata-only".to_owned(),
        };
        return Ok(resolved);
    }

    if profile.spec.is_some_and(|spec| spec > 1) {
        return Err(loader_error(
            ErrorCode::LoaderInstallerSpecUnsupported,
            "loader installer profile spec is newer than the supported processor family",
        ));
    }

    if profile.minecraft.as_deref() != Some(resolved.minecraft.as_str()) {
        return Err(loader_error(
            ErrorCode::LoaderProfileInvalid,
            "loader installer profile targets a different Minecraft base version",
        ));
    }

    if profile.processors.len() > MAX_PROCESSORS || profile.libraries.len() > MAX_PROFILE_LIBRARIES
    {
        return Err(loader_error(
            ErrorCode::LoaderInstallerInvalid,
            "loader installer profile exceeds resource bounds",
        ));
    }

    let version_bytes = selected.get(VERSION_JSON).ok_or_else(|| {
        loader_error(
            ErrorCode::LoaderInstallerSpecUnsupported,
            "modern loader installer does not contain version.json",
        )
    })?;

    if version_bytes.len() > crate::loader::common::MAX_LOADER_METADATA_BYTES {
        return Err(loader_error(
            ErrorCode::LoaderProfileInvalid,
            "loader version metadata exceeds its size bound",
        ));
    }

    let version: VersionDto = serde_json::from_slice(version_bytes).map_err(|source| {
        loader_error(
            ErrorCode::LoaderProfileInvalid,
            "loader version metadata inside installer is invalid JSON",
        )
        .with_source(source)
    })?;

    if version.inherits_from.as_deref() != Some(resolved.minecraft.as_str()) {
        return Err(loader_error(
            ErrorCode::LoaderProfileInvalid,
            "loader version metadata inherits from a different Minecraft base version",
        ));
    }

    let mut dependency_libraries = BTreeMap::<String, ResolvedArtifact>::new();
    for library in &profile.libraries {
        let (coordinate, resolved_artifact) =
            normalize_remote_library(library, default_maven_base, allow_http)?;
        dependency_libraries.insert(coordinate.to_string_key(), resolved_artifact);
    }

    let data = normalize_data(&profile.data, &name_set)?;
    let installer_id = resolved
        .preparation
        .installer
        .as_ref()
        .ok_or_else(|| {
            loader_error(
                ErrorCode::LoaderInstallerInvalid,
                "verified loader installer identity was lost before normalization",
            )
        })?
        .artifact
        .id;
    let NormalizedProcessors {
        processors,
        generated_outputs,
        generated_by_coordinate,
    } = normalize_processors(
        &profile.processors,
        &dependency_libraries,
        &data,
        installer_id,
    )?;
    let input_artifacts = collect_processor_inputs(
        &processors,
        &data,
        &dependency_libraries,
        &generated_by_coordinate,
    )?;
    let main_class = version.main_class.clone();
    let java_version = version.java_version.clone();
    let (jvm_args, game_args) = normalize_version_arguments(&version)?;
    let libraries = normalize_version_libraries(
        version.libraries,
        default_maven_base,
        allow_http,
        &generated_by_coordinate,
    )?;

    if let Some(main_class) = main_class {
        validate_text(&main_class, 512, "loader main class is invalid")?;
        resolved.patch.main_class = Some(main_class);
    }

    resolved.patch.libraries = libraries;
    resolved.patch.jvm_args = jvm_args;
    resolved.patch.game_args = game_args;

    if let Some(java) = java_version
        && let Some(major_version) = java.major_version
    {
        if major_version == 0 || major_version > 255 {
            return Err(loader_error(
                ErrorCode::LoaderProfileInvalid,
                "loader Java requirement is invalid",
            ));
        }
        resolved.patch.java_requirement = Some(MinecraftJavaRequirement {
            major_version,
            component_hint: java.component,
        });
    }

    let installer = resolved.preparation.installer.as_ref().ok_or_else(|| {
        loader_error(
            ErrorCode::LoaderInstallerInvalid,
            "verified loader installer identity was lost before provenance normalization",
        )
    })?;
    let installer_digest = installer
        .artifact
        .integrity
        .sha256()
        .map(|value| format!("sha256={value}"))
        .or_else(|| {
            installer
                .artifact
                .integrity
                .sha1()
                .map(|value| format!("sha1={value}"))
        })
        .ok_or_else(|| {
            loader_error(
                ErrorCode::LoaderArtifactUnverifiable,
                "verified loader installer has no persisted integrity identity",
            )
        })?;

    resolved.patch.component.provenance.detail = Some(format!(
        "modern-processor-profile;spec={};installer_id={};installer_{installer_digest}",
        profile.spec.unwrap_or(1),
        installer.artifact.id
    ));

    let embedded_inputs = collect_embedded_inputs(&data, &name_set, &processors)?;
    resolved.preparation = ComponentPreparationRecipe {
        component: Some(resolved.patch.component.clone()),
        installer: resolved.preparation.installer.take(),
        embedded_inputs,
        input_artifacts,
        data,
        processors,
        generated_outputs,
        installer_java_requirement: None,
    };
    resolved.support = LoaderSupport::Supported;

    Ok(resolved)
}
