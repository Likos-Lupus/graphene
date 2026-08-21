use super::*;

pub(super) struct NormalizedProcessors {
    pub(super) processors: Vec<ProcessorStep>,
    pub(super) generated_outputs: Vec<GeneratedOutput>,
    pub(super) generated_by_coordinate: BTreeMap<String, GeneratedOutput>,
}

pub(super) fn normalize_processors(
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
                "loader processor exceeds resource bounds",
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

pub(super) fn collect_processor_inputs(
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
