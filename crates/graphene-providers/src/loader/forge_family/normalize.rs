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
            reason: "legacy launcher-profile installer family is not Tier A in Phase 3".to_owned(),
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
            "loader installer profile exceeds Phase 3 resource bounds",
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

trait CoordinateKey {
    fn to_string_key(&self) -> String;
}

impl CoordinateKey for MavenCoordinate {
    fn to_string_key(&self) -> String {
        let classifier = self
            .classifier
            .as_deref()
            .map(|v| format!(":{v}"))
            .unwrap_or_default();
        let extension = if self.extension == "jar" {
            String::new()
        } else {
            format!("@{}", self.extension)
        };

        format!(
            "{}:{}:{}{}{}",
            self.group, self.artifact, self.version, classifier, extension
        )
    }
}

fn normalize_remote_library(
    library: &LibraryDto,
    default_maven_base: &str,
    allow_http: bool,
) -> Result<(MavenCoordinate, ResolvedArtifact)> {
    let coordinate = MavenCoordinate::parse(&library.name).map_err(|source| {
        loader_error(
            ErrorCode::LoaderProfileInvalid,
            "loader profile contains an invalid Maven coordinate",
        )
        .with_source(source)
    })?;
    let repository_path = coordinate.repository_path()?;
    let download = library
        .downloads
        .as_ref()
        .and_then(|downloads| downloads.artifact.as_ref());
    let resolved = if let Some(download) = download {
        remote_download(
            download,
            &repository_path,
            library.url.as_deref().unwrap_or(default_maven_base),
            allow_http,
        )?
    } else {
        return Err(loader_error(
            ErrorCode::LoaderArtifactUnverifiable,
            "loader processor dependency lacks verified download metadata",
        ));
    };

    Ok((coordinate, resolved))
}

fn remote_download(
    download: &DownloadDto,
    repository_path: &ManagedPath,
    repository_base: &str,
    allow_http: bool,
) -> Result<ResolvedArtifact> {
    let relative = match &download.path {
        Some(path) => {
            let path = ManagedPath::new(path.clone()).map_err(|source| {
                loader_error(
                    ErrorCode::LoaderProfileInvalid,
                    "loader artifact path is unsafe",
                )
                .with_source(source)
            })?;

            if path != *repository_path {
                return Err(loader_error(
                    ErrorCode::LoaderProfileInvalid,
                    "loader artifact path does not match its Maven coordinate",
                ));
            }
            path
        }

        None => repository_path.clone(),
    };

    let url = download
        .url
        .clone()
        .unwrap_or_else(|| join_base(repository_base, relative.as_str()));
    let mut integrity = ArtifactIntegrity::none();

    if let Some(value) = &download.sha1 {
        integrity = integrity.with_sha1(value.parse().map_err(|source| {
            loader_error(
                ErrorCode::LoaderProfileInvalid,
                "loader artifact SHA-1 is invalid",
            )
            .with_source(source)
        })?);
    }

    if let Some(value) = &download.sha256 {
        integrity = integrity.with_sha256(value.parse().map_err(|source| {
            loader_error(
                ErrorCode::LoaderProfileInvalid,
                "loader artifact SHA-256 is invalid",
            )
            .with_source(source)
        })?);
    }

    Ok(ResolvedArtifact {
        artifact: artifact(url, integrity, download.size, allow_http)?,
        relative_path: ManagedPath::new(format!("shared/libraries/{}", relative.as_str()))?,
    })
}

fn normalize_data(
    values: &BTreeMap<String, DataFileDto>,
    entries: &BTreeSet<&str>,
) -> Result<BTreeMap<String, PreparationDataValue>> {
    if values.len() > 1024 {
        return Err(loader_error(
            ErrorCode::LoaderInstallerInvalid,
            "loader installer data map exceeds its bound",
        ));
    }

    values
        .iter()
        .map(|(name, value)| {
            validate_key(name)?;
            Ok((
                name.clone(),
                PreparationDataValue::SideSpecific {
                    client: Box::new(parse_data_value(&value.client, entries)?),
                    server: value
                        .server
                        .as_deref()
                        .map(|value| parse_data_value(value, entries).map(Box::new))
                        .transpose()?,
                },
            ))
        })
        .collect()
}

fn parse_data_value(value: &str, entries: &BTreeSet<&str>) -> Result<PreparationDataValue> {
    validate_text(value, 16 * 1024, "loader installer data value is invalid")?;
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return Ok(PreparationDataValue::Literal(
            value[1..value.len() - 1].to_owned(),
        ));
    }

    if value.len() >= 2 && value.starts_with('[') && value.ends_with(']') {
        return MavenCoordinate::parse(&value[1..value.len() - 1])
            .map(PreparationDataValue::MavenCoordinate)
            .map_err(|source| {
                loader_error(
                    ErrorCode::LoaderInstallerInvalid,
                    "loader installer data contains an invalid Maven coordinate",
                )
                .with_source(source)
            });
    }

    if entries.contains(value) {
        return Ok(PreparationDataValue::EmbeddedInstallerEntry(
            value.to_owned(),
        ));
    }

    Ok(PreparationDataValue::Literal(value.to_owned()))
}

struct NormalizedProcessors {
    processors: Vec<ProcessorStep>,
    generated_outputs: Vec<GeneratedOutput>,
    generated_by_coordinate: BTreeMap<String, GeneratedOutput>,
}

fn normalize_processors(
    values: &[ProcessorDto],
    libraries: &BTreeMap<String, ResolvedArtifact>,
    data: &BTreeMap<String, PreparationDataValue>,
    installer_id: ArtifactId,
) -> Result<NormalizedProcessors> {
    let mut processors = Vec::new();
    let mut outputs = Vec::new();
    let mut generated_by_coordinate = BTreeMap::new();

    for (index, processor) in values.iter().enumerate() {
        let side = normalize_side(&processor.sides)?;
        if !side.executes_for_client() {
            continue;
        }

        if processor.classpath.len() > MAX_PROCESSOR_CLASSPATH
            || processor.args.len() > 4096
            || processor.outputs.len() > MAX_PROCESSOR_OUTPUTS
        {
            return Err(loader_error(
                ErrorCode::LoaderProcessorUnsupported,
                "loader processor exceeds Phase 3 bounds",
            ));
        }

        let executable_coordinate = parse_bracket_coordinate(&processor.jar)?;
        let executable_jar = lookup_library(libraries, &executable_coordinate)?;
        let classpath = processor
            .classpath
            .iter()
            .map(|value| {
                let coordinate = parse_bracket_coordinate(value)?;
                lookup_library(libraries, &coordinate)
            })
            .collect::<Result<Vec<_>>>()?;
        let arguments = processor
            .args
            .iter()
            .map(|value| parse_argument(value))
            .collect::<Result<Vec<_>>>()?;
        let processor_id = format!("processor-{index:03}-{}", executable_coordinate.artifact);
        let mut declared_outputs = Vec::new();

        for (output_index, (target, digest)) in processor.outputs.iter().enumerate() {
            let coordinate = output_coordinate(target, data)?;
            let repository_path = coordinate.repository_path()?;
            let digest = output_digest(digest, data)?;
            let integrity =
                ArtifactIntegrity::none().with_sha1(digest.parse().map_err(|source| {
                    loader_error(
                        ErrorCode::LoaderProcessorOutputMismatch,
                        "loader processor declared output SHA-1 is invalid",
                    )
                    .with_source(source)
                })?);
            let id = format!("{}-output-{output_index:03}", processor_id);
            let input_identity =
                processor_input_identity(installer_id, &executable_jar, &classpath, &arguments)?;
            let output = GeneratedOutput {
                id: id.clone(),
                producer: processor_id.clone(),
                input_identity,
                staging_path: ManagedPath::new(format!(
                    "root/libraries/{}",
                    repository_path.as_str()
                ))?,
                managed_destination: ManagedPath::new(format!(
                    "shared/libraries/{}",
                    repository_path.as_str()
                ))?,
                scope: GeneratedOutputScope::SharedImmutable,
                expected_size: None,
                expected_integrity: integrity,
            };

            if generated_by_coordinate
                .insert(coordinate.to_string_key(), output.clone())
                .is_some()
            {
                return Err(loader_error(
                    ErrorCode::LoaderProcessorUnsupported,
                    "multiple loader processors declare the same generated Maven output",
                ));
            }

            declared_outputs.push(id);
            outputs.push(output);
        }

        processors.push(ProcessorStep {
            id: processor_id,
            java_requirement: None,
            executable_jar,
            classpath,
            main_class: None,
            arguments,
            side,
            declared_outputs,
            timeout_seconds: DEFAULT_PROCESSOR_TIMEOUT_SECONDS,
        });
    }

    Ok(NormalizedProcessors {
        processors,
        generated_outputs: outputs,
        generated_by_coordinate,
    })
}

fn processor_input_identity(
    installer_id: ArtifactId,
    executable: &ResolvedArtifact,
    classpath: &[ResolvedArtifact],
    arguments: &[graphene_minecraft::PreparationArgument],
) -> Result<String> {
    let arguments = serde_json::to_string(arguments).map_err(|source| {
        loader_error(
            ErrorCode::LoaderProcessorUnsupported,
            "failed to normalize loader processor input identity",
        )
        .with_source(source)
    })?;

    let classpath = classpath
        .iter()
        .map(|artifact| artifact.artifact.id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let identity = format!(
        "installer={installer_id};jar={};classpath={classpath};args={arguments}",
        executable.artifact.id
    );

    if identity.len() > 16 * 1024 || identity.contains('\0') {
        return Err(loader_error(
            ErrorCode::LoaderProcessorUnsupported,
            "loader processor input identity exceeds its bound",
        ));
    }

    Ok(identity)
}

fn normalize_side(values: &[String]) -> Result<ProcessorSideCondition> {
    if values.is_empty() {
        return Ok(ProcessorSideCondition::Any);
    }

    let has_client = values.iter().any(|value| value == "client");
    let has_server = values.iter().any(|value| value == "server");

    if values
        .iter()
        .any(|value| value != "client" && value != "server")
    {
        return Err(loader_error(
            ErrorCode::LoaderProcessorUnsupported,
            "loader processor contains an unknown side restriction",
        ));
    }

    Ok(if has_client {
        ProcessorSideCondition::Client
    } else if has_server {
        ProcessorSideCondition::Server
    } else {
        ProcessorSideCondition::Any
    })
}

fn parse_bracket_coordinate(value: &str) -> Result<MavenCoordinate> {
    if value.len() < 3 || !value.starts_with('[') || !value.ends_with(']') {
        return Err(loader_error(
            ErrorCode::LoaderProcessorUnsupported,
            "loader processor executable/classpath is not a Maven artifact reference",
        ));
    }

    MavenCoordinate::parse(&value[1..value.len() - 1]).map_err(|source| {
        loader_error(
            ErrorCode::LoaderProcessorUnsupported,
            "loader processor contains an invalid Maven artifact reference",
        )
        .with_source(source)
    })
}

fn lookup_library(
    libraries: &BTreeMap<String, ResolvedArtifact>,
    coordinate: &MavenCoordinate,
) -> Result<ResolvedArtifact> {
    libraries
        .get(&coordinate.to_string_key())
        .cloned()
        .ok_or_else(|| {
            loader_error(
                ErrorCode::LoaderProcessorUnsupported,
                "loader processor dependency is not declared by the verified installer profile",
            )
        })
}

fn output_coordinate(
    value: &str,
    data: &BTreeMap<String, PreparationDataValue>,
) -> Result<MavenCoordinate> {
    let raw = if value.starts_with('{') && value.ends_with('}') {
        let name = &value[1..value.len() - 1];
        data_client_raw(data.get(name).ok_or_else(|| {
            loader_error(
                ErrorCode::LoaderProcessorOutputMissing,
                "loader processor output references missing installer data",
            )
        })?)?
    } else {
        value.to_owned()
    };

    if raw.len() < 3 || !raw.starts_with('[') || !raw.ends_with(']') {
        return Err(loader_error(
            ErrorCode::LoaderProcessorUnsupported,
            "loader processor output is not a managed Maven destination",
        ));
    }

    MavenCoordinate::parse(&raw[1..raw.len() - 1]).map_err(|source| {
        loader_error(
            ErrorCode::LoaderProcessorUnsupported,
            "loader processor output Maven destination is invalid",
        )
        .with_source(source)
    })
}

fn output_digest(value: &str, data: &BTreeMap<String, PreparationDataValue>) -> Result<String> {
    let raw = if value.starts_with('{') && value.ends_with('}') {
        let name = &value[1..value.len() - 1];
        data_client_raw(data.get(name).ok_or_else(|| {
            loader_error(
                ErrorCode::LoaderProcessorOutputMismatch,
                "loader processor output digest references missing installer data",
            )
        })?)?
    } else {
        value.to_owned()
    };

    let value = raw.trim_matches('\'');
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(loader_error(
            ErrorCode::LoaderProcessorOutputMismatch,
            "loader processor output digest is not a SHA-1 value",
        ));
    }

    Ok(value.to_ascii_lowercase())
}

fn data_client_raw(value: &PreparationDataValue) -> Result<String> {
    let value = match value {
        PreparationDataValue::SideSpecific { client, .. } => client.as_ref(),
        other => other,
    };

    Ok(match value {
        PreparationDataValue::Literal(value) => format!("'{value}'"),
        PreparationDataValue::MavenCoordinate(value) => format!("[{}]", value.to_string_key()),
        PreparationDataValue::EmbeddedInstallerEntry(value) => value.clone(),
        PreparationDataValue::ManagedPath(value) => value.as_str().to_owned(),
        PreparationDataValue::SideSpecific { .. } => {
            return Err(loader_error(
                ErrorCode::LoaderInstallerInvalid,
                "loader installer data contains nested side-specific data",
            ));
        }
        _ => {
            return Err(loader_error(
                ErrorCode::LoaderInstallerInvalid,
                "loader installer data variant is unsupported",
            ));
        }
    })
}

fn collect_processor_inputs(
    processors: &[ProcessorStep],
    data: &BTreeMap<String, PreparationDataValue>,
    libraries: &BTreeMap<String, ResolvedArtifact>,
    generated: &BTreeMap<String, GeneratedOutput>,
) -> Result<Vec<ResolvedArtifact>> {
    let mut coordinates = BTreeSet::<String>::new();

    for value in data.values() {
        collect_data_maven(value, &mut coordinates);
    }

    for processor in processors {
        for argument in &processor.arguments {
            collect_argument_maven(argument, data, &mut coordinates)?;
        }
    }

    let mut artifacts = Vec::new();
    let mut ids = HashSet::new();

    for coordinate in coordinates {
        if generated.contains_key(&coordinate) {
            continue;
        }

        let artifact = libraries.get(&coordinate).ok_or_else(|| {
            loader_error(
                ErrorCode::LoaderProcessorUnsupported,
                "loader processor references a Maven input not declared by the verified profile",
            )
            .with_context("coordinate", coordinate.clone())
        })?;

        if ids.insert(artifact.artifact.id) {
            artifacts.push(artifact.clone());
        }
    }

    artifacts.sort_by(|left, right| {
        left.relative_path
            .as_str()
            .cmp(right.relative_path.as_str())
    });

    Ok(artifacts)
}

fn collect_data_maven(value: &PreparationDataValue, output: &mut BTreeSet<String>) {
    match value {
        PreparationDataValue::MavenCoordinate(value) => {
            output.insert(value.to_string_key());
        }
        PreparationDataValue::SideSpecific { client, .. } => collect_data_maven(client, output),
        _ => {}
    }
}

fn collect_argument_maven(
    argument: &graphene_minecraft::PreparationArgument,
    data: &BTreeMap<String, PreparationDataValue>,
    output: &mut BTreeSet<String>,
) -> Result<()> {
    use graphene_minecraft::{
        PreparationArgument, PreparationArgumentPart, PreparationPlaceholder,
    };

    fn placeholder(
        value: &PreparationPlaceholder,
        data: &BTreeMap<String, PreparationDataValue>,
        output: &mut BTreeSet<String>,
    ) -> Result<()> {
        match value {
            PreparationPlaceholder::MavenPath(value) => {
                output.insert(value.to_string_key());
            }
            PreparationPlaceholder::Data(name) => {
                let value = data.get(name).ok_or_else(|| {
                    loader_error(
                        ErrorCode::LoaderProcessorPlaceholderInvalid,
                        "loader processor data placeholder is missing during normalization",
                    )
                })?;
                collect_data_maven(value, output);
            }
            _ => {}
        }
        Ok(())
    }

    match argument {
        PreparationArgument::Placeholder(value) => placeholder(value, data, output),
        PreparationArgument::Template(parts) => {
            for part in parts {
                if let PreparationArgumentPart::Placeholder(value) = part {
                    placeholder(value, data, output)?;
                }
            }
            Ok(())
        }
        PreparationArgument::Literal(_) => Ok(()),
        _ => Err(loader_error(
            ErrorCode::LoaderProcessorUnsupported,
            "loader processor argument variant is unsupported",
        )),
    }
}

fn normalize_version_libraries(
    values: Vec<LibraryDto>,
    default_maven_base: &str,
    allow_http: bool,
    generated: &BTreeMap<String, GeneratedOutput>,
) -> Result<Vec<Library>> {
    if values.len() > MAX_PROFILE_LIBRARIES {
        return Err(loader_error(
            ErrorCode::LoaderProfileInvalid,
            "loader version metadata contains too many libraries",
        ));
    }

    values
        .into_iter()
        .map(|library| {
            let coordinate = MavenCoordinate::parse(&library.name).map_err(|source| {
                loader_error(
                    ErrorCode::LoaderProfileInvalid,
                    "loader version metadata contains an invalid Maven coordinate",
                )
                    .with_source(source)
            })?;

            let rules = library
                .rules
                .iter()
                .map(normalize_rule)
                .collect::<Result<Vec<_>>>()?;

            let artifact = if let Some(download) = library
                .downloads
                .as_ref()
                .and_then(|downloads| downloads.artifact.as_ref())
            {
                Some(remote_download(
                    download,
                    &coordinate.repository_path()?,
                    library.url.as_deref().unwrap_or(default_maven_base),
                    allow_http,
                )?)
            } else if let Some(output) = generated.get(&coordinate.to_string_key()) {
                Some(generated_resolved_artifact(output)?)
            } else {
                return Err(loader_error(
                    ErrorCode::LoaderArtifactUnverifiable,
                    "loader runtime library is neither a verified remote artifact nor a declared generated output",
                ));
            };

            Ok(Library {
                coordinate,
                rules,
                artifact,
                classifiers: BTreeMap::new(),
                natives: BTreeMap::new(),
            })
        })
        .collect()
}

fn generated_resolved_artifact(output: &GeneratedOutput) -> Result<ResolvedArtifact> {
    if !output.expected_integrity.is_verifiable() {
        return Err(loader_error(
            ErrorCode::LoaderArtifactUnverifiable,
            "generated loader runtime artifact has no declared digest",
        ));
    }

    let mut id = [0_u8; 16];

    if let Some(sha256) = output.expected_integrity.sha256() {
        id.copy_from_slice(&sha256.as_bytes()[..16]);
    } else if let Some(sha1) = output.expected_integrity.sha1() {
        id.copy_from_slice(&sha1.as_bytes()[..16]);
    }

    Ok(ResolvedArtifact {
        artifact: Artifact {
            id: ArtifactId::from_bytes(id),
            kind: ArtifactKind::Binary,
            sources: Vec::new(),
            integrity: output.expected_integrity.clone(),
            expected_size: output.expected_size,
            cache_policy: graphene_core::CachePolicy::UseVerified,
        },
        relative_path: output.managed_destination.clone(),
    })
}

fn normalize_version_arguments(version: &VersionDto) -> Result<(Vec<Argument>, Vec<Argument>)> {
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

fn normalize_rule(value: &RuleDto) -> Result<Rule> {
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

fn collect_embedded_inputs(
    data: &BTreeMap<String, PreparationDataValue>,
    entries: &BTreeSet<&str>,
    processors: &[ProcessorStep],
) -> Result<Vec<graphene_minecraft::EmbeddedInstallerInput>> {
    let mut unique = BTreeSet::new();
    let mut data_entries = BTreeMap::<String, String>::new();

    for (name, value) in data {
        collect_embedded_value(value, &mut unique)?;
        if let Some(entry) = client_embedded_entry(value) {
            data_entries.insert(name.clone(), entry.to_owned());
        }
    }

    let mut consumers = BTreeMap::<String, BTreeSet<String>>::new();
    for processor in processors {
        let mut names = BTreeSet::new();
        for argument in &processor.arguments {
            collect_argument_data_names(argument, &mut names)?;
        }

        for name in names {
            if let Some(entry) = data_entries.get(&name) {
                consumers
                    .entry(entry.clone())
                    .or_default()
                    .insert(processor.id.clone());
            }
        }
    }

    unique
        .into_iter()
        .map(|entry| {
            if !entries.contains(entry.as_str()) {
                return Err(loader_error(
                    ErrorCode::LoaderInstallerInvalid,
                    "loader embedded input is missing from the verified installer",
                ));
            }

            Ok(graphene_minecraft::EmbeddedInstallerInput {
                staging_path: ManagedPath::new(format!("inputs/{entry}"))?,
                entry: entry.clone(),
                expected_integrity: ArtifactIntegrity::none(),
                maximum_size: jar::MAX_INSTALLER_ENTRY_BYTES as u64,
                consumers: consumers
                    .remove(&entry)
                    .unwrap_or_default()
                    .into_iter()
                    .collect(),
            })
        })
        .collect()
}

fn client_embedded_entry(value: &PreparationDataValue) -> Option<&str> {
    match value {
        PreparationDataValue::EmbeddedInstallerEntry(value) => Some(value.as_str()),
        PreparationDataValue::SideSpecific { client, .. } => client_embedded_entry(client),
        _ => None,
    }
}

fn collect_argument_data_names(
    argument: &graphene_minecraft::PreparationArgument,
    output: &mut BTreeSet<String>,
) -> Result<()> {
    use graphene_minecraft::{
        PreparationArgument, PreparationArgumentPart, PreparationPlaceholder,
    };

    let mut visit = |value: &PreparationPlaceholder| {
        if let PreparationPlaceholder::Data(name) = value {
            output.insert(name.clone());
        }
    };

    match argument {
        PreparationArgument::Placeholder(value) => visit(value),
        PreparationArgument::Template(parts) => {
            for part in parts {
                if let PreparationArgumentPart::Placeholder(value) = part {
                    visit(value);
                }
            }
        }
        PreparationArgument::Literal(_) => {}
        _ => {
            return Err(loader_error(
                ErrorCode::LoaderProcessorUnsupported,
                "loader processor argument variant is unsupported",
            ));
        }
    }

    Ok(())
}

fn collect_embedded_value(
    value: &PreparationDataValue,
    output: &mut BTreeSet<String>,
) -> Result<()> {
    match value {
        PreparationDataValue::EmbeddedInstallerEntry(value) => {
            output.insert(value.clone());
        }
        PreparationDataValue::SideSpecific { client, .. } => {
            collect_embedded_value(client, output)?;
        }
        _ => {}
    }

    Ok(())
}

fn validate_key(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(loader_error(
            ErrorCode::LoaderInstallerInvalid,
            "loader installer data key is invalid",
        ));
    }

    Ok(())
}

fn validate_text(value: &str, maximum: usize, message: &'static str) -> Result<()> {
    if value.is_empty() || value.len() > maximum || value.contains('\0') {
        return Err(loader_error(ErrorCode::LoaderProfileInvalid, message));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphene_core::{ArtifactIntegrity, OperationRegistry};
    use graphene_minecraft::{
        ComponentConflict, ComponentDescriptor, ComponentKind, ComponentProvenance,
        ComponentRequirement, ComponentUid, ComponentVersion, LoaderVersion, MinecraftVersionId,
        MinecraftVersionPatch, ResolvedComponent,
    };
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/loaders")
            .join(name)
    }

    fn shell(kind: LoaderKind, minecraft: &str) -> ResolvedLoader {
        let uid = ComponentUid::new(kind.component_uid()).expect("component uid");
        let version = LoaderVersion::new("fixture-loader").expect("loader version");
        let component_version = ComponentVersion::new(version.as_str()).expect("component version");
        let descriptor = ComponentDescriptor {
            uid: uid.clone(),
            version: component_version.clone(),
            kind: ComponentKind::Loader,
            order: 100,
            requires: vec![ComponentRequirement::Exact {
                uid: ComponentUid::new("net.minecraft").expect("minecraft uid"),
                version: ComponentVersion::new(minecraft).expect("minecraft version"),
            }],
            conflicts: [LoaderKind::Fabric, LoaderKind::Forge, LoaderKind::NeoForge]
                .into_iter()
                .filter(|candidate| *candidate != kind)
                .map(|candidate| ComponentConflict {
                    uid: ComponentUid::new(candidate.component_uid()).expect("conflict uid"),
                })
                .collect(),
        };
        let component = ResolvedComponent {
            uid,
            version: component_version,
            kind: ComponentKind::Loader,
            provenance: ComponentProvenance::new(kind.provider_id(), Some("fixture".to_owned()))
                .expect("provenance"),
        };
        let integrity = ArtifactIntegrity::none().with_sha1(
            "d83ef5c6f32a695513e248ff8eee0996c1b31125"
                .parse()
                .expect("sha1"),
        );
        let installer = ResolvedArtifact {
            artifact: artifact(
                "https://example.invalid/installer.jar".to_owned(),
                integrity,
                None,
                false,
            )
            .expect("installer"),
            relative_path: ManagedPath::new("shared/loader-installers/fixture/installer.jar")
                .expect("managed path"),
        };
        ResolvedLoader {
            kind,
            version,
            minecraft: MinecraftVersionId::new(minecraft).expect("minecraft"),
            component: descriptor,
            patch: MinecraftVersionPatch::empty(component.clone()),
            preparation: ComponentPreparationRecipe {
                component: Some(component),
                installer: Some(installer),
                ..Default::default()
            },
            support: LoaderSupport::MetadataOnly {
                reason: "fixture shell".to_owned(),
            },
        }
    }

    fn normalize(name: &str, family: ForgeFamily) -> Result<ResolvedLoader> {
        let registry = OperationRegistry::new(16).expect("operation registry");
        let operation = registry.create("normalize-fixture");
        normalize_verified_installer(
            shell(family.kind(), "1.21.1"),
            &fixture(name),
            &operation,
            family,
            "https://maven.example.invalid",
            false,
        )
    }

    #[test]
    fn modern_forge_profile_normalizes_into_patch_and_recipe() {
        let resolved =
            normalize("forge-modern-fixture.jar", ForgeFamily::Forge).expect("normalize");
        assert_eq!(resolved.support, LoaderSupport::Supported);
        assert_eq!(
            resolved.patch.main_class.as_deref(),
            Some("net.minecraftforge.bootstrap.ForgeBootstrap")
        );
        assert_eq!(
            resolved.preparation.processors.len(),
            1,
            "server-only processor is skipped"
        );
        assert_eq!(resolved.preparation.generated_outputs.len(), 1);
        assert!(
            resolved.preparation.generated_outputs[0]
                .expected_integrity
                .is_verifiable()
        );
        assert!(
            !resolved.preparation.generated_outputs[0]
                .input_identity
                .is_empty()
        );
        assert_eq!(
            resolved
                .patch
                .java_requirement
                .as_ref()
                .map(|value| value.major_version),
            Some(21)
        );
    }

    #[test]
    fn neoforge_converges_on_same_preparation_model() {
        let resolved = normalize("neoforge-modern-fixture.jar", ForgeFamily::NeoForge)
            .expect("normalize NeoForge");
        assert_eq!(resolved.kind, LoaderKind::NeoForge);
        assert_eq!(resolved.support, LoaderSupport::Supported);
        assert_eq!(resolved.preparation.processors.len(), 1);
        assert_eq!(resolved.preparation.generated_outputs.len(), 1);
    }

    #[test]
    fn embedded_client_input_is_declared_with_consumers() {
        let resolved = normalize("forge-modern-embedded-fixture.jar", ForgeFamily::Forge)
            .expect("normalize embedded fixture");
        let embedded = resolved
            .preparation
            .embedded_inputs
            .iter()
            .find(|entry| entry.entry == "data/client.lzma")
            .expect("client embedded input");
        assert!(!embedded.consumers.is_empty());
        assert!(
            resolved
                .preparation
                .embedded_inputs
                .iter()
                .all(|entry| entry.entry != "data/server.lzma"),
            "server-only data must not enter the client recipe"
        );
    }

    #[test]
    fn legacy_profile_is_metadata_only_not_modern_fallback() {
        let registry = OperationRegistry::new(16).expect("operation registry");
        let operation = registry.create("legacy-fixture");
        let resolved = normalize_verified_installer(
            shell(LoaderKind::Forge, "1.12.2"),
            &fixture("forge-legacy-fixture.jar"),
            &operation,
            ForgeFamily::Forge,
            "https://maven.example.invalid",
            false,
        )
        .expect("legacy classification");
        assert!(matches!(
            resolved.support,
            LoaderSupport::MetadataOnly { .. }
        ));
        assert!(resolved.preparation.processors.is_empty());
    }

    #[test]
    fn hostile_installer_fixtures_fail_before_execution() {
        for name in [
            "hostile/traversal.jar",
            "hostile/absolute.jar",
            "hostile/malformed-json.jar",
            "hostile/duplicate-profile.jar",
            "hostile/unknown-placeholder.jar",
            "hostile/output-escape.jar",
            "hostile/nul-argument.jar",
            "hostile/script-reference.jar",
            "hostile/unsafe-repository.jar",
            "hostile/unsupported-spec.jar",
            "hostile/base-mismatch.jar",
            "hostile/oversized-metadata.jar",
            "hostile/oversized-embedded.jar",
            "hostile/too-many-entries.jar",
            "hostile/symlink-entry.jar",
            "hostile/special-file.jar",
        ] {
            assert!(fixture(name).is_file(), "missing hostile fixture: {name}");
            assert!(
                normalize(name, ForgeFamily::Forge).is_err(),
                "{name} must fail"
            );
        }
    }
}
