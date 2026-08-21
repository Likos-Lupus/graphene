use super::{materialize::acquired_source, spawn_blocking_install};
use crate::{
    acquisition::AcquiredArtifact,
    archive,
    error::{cancelled_error, install_error},
    r#mod::{
        locally_derived_sha256, reusable_generated_output, verify_and_publish_generated_output,
    },
    path::minecraft_path_to_platform,
    plan::InstallPlan,
    processor::{
        InstallToolRunner, ProcessorExpansionContext, ToolRunRequest, expand_processor_arguments,
    },
};
use graphene_core::{
    ArtifactId, CancellationToken, ErrorCode, OperationController, Progress, Result,
};
use graphene_minecraft::{GeneratedOutputScope, ResolvedArtifact, ResolvedComponent};
use graphene_platform::ManagedRelativePath;
use graphene_storage::DataRoot;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

const COPY_BUFFER_BYTES: usize = 256 * 1024;

pub(super) async fn execute_preparation(
    data_root: &DataRoot,
    plan: &InstallPlan,
    acquired: &HashMap<ArtifactId, AcquiredArtifact>,
    runner: Option<&Arc<dyn InstallToolRunner>>,
    operation: &OperationController,
    staging_root: &Path,
) -> Result<()> {
    if plan.preparation.processors.is_empty()
        && plan.preparation.embedded_inputs.is_empty()
        && plan.preparation.generated_outputs.is_empty()
    {
        return Ok(());
    }

    let component = plan.preparation.component.as_ref().ok_or_else(|| {
        install_error(
            ErrorCode::InstallPlanInvalid,
            "loader preparation is missing exact component provenance",
        )
    })?;

    let runner = runner.ok_or_else(|| {
        install_error(
            ErrorCode::LoaderProcessorJavaUnavailable,
            "loader preparation requires an install-tool runner",
        )
    })?;

    let installer = plan.preparation.installer.as_ref().ok_or_else(|| {
        install_error(
            ErrorCode::LoaderInstallerInvalid,
            "loader preparation requires a verified installer artifact",
        )
    })?;

    checkpoint(operation)?;

    operation.set_stage("prepare-loader-staging")?;
    let work_root = staging_root.join(".graphene-loader-work");
    if work_root.exists() {
        return Err(install_error(
            ErrorCode::InstallStageFailed,
            "loader preparation staging already exists",
        ));
    }

    fs::create_dir(&work_root).map_err(|source| {
        install_error(
            ErrorCode::InstallStageFailed,
            "failed to create loader preparation staging",
        )
        .with_source(source)
    })?;

    let result = execute_preparation_inner(PreparationExecution {
        data_root,
        plan,
        acquired,
        runner,
        operation,
        work_root: &work_root,
        installer,
        component,
    })
    .await;
    let cleanup = tokio::fs::remove_dir_all(&work_root).await;

    if result.is_ok() {
        cleanup.map_err(|source| {
            install_error(
                ErrorCode::InstallStageFailed,
                "failed to remove loader preparation staging",
            )
            .with_source(source)
        })?;
    }

    result
}

struct PreparationExecution<'a> {
    data_root: &'a DataRoot,
    plan: &'a InstallPlan,
    acquired: &'a HashMap<ArtifactId, AcquiredArtifact>,
    runner: &'a Arc<dyn InstallToolRunner>,
    operation: &'a OperationController,
    work_root: &'a Path,
    installer: &'a ResolvedArtifact,
    component: &'a ResolvedComponent,
}

async fn execute_preparation_inner(context: PreparationExecution<'_>) -> Result<()> {
    let PreparationExecution {
        data_root,
        plan,
        acquired,
        runner,
        operation,
        work_root,
        installer,
        component,
    } = context;
    let root = work_root.join("root");
    let library_dir = root.join("libraries");
    let installer_dir = work_root.join("installer");
    let input_dir = work_root.join("inputs");

    fs::create_dir_all(&library_dir).map_err(stage_error)?;
    fs::create_dir_all(&installer_dir).map_err(stage_error)?;
    fs::create_dir_all(&input_dir).map_err(stage_error)?;

    let installer_path = installer_dir.join("installer.jar");
    copy_private(
        acquired_source(acquired, installer.artifact.id)?,
        &installer_path,
        &operation.cancellation_token(),
    )?;

    let minecraft_version = plan.minecraft.version_id.as_str();
    let minecraft_jar = root
        .join("versions")
        .join(minecraft_version)
        .join(format!("{minecraft_version}.jar"));
    copy_private(
        acquired_source(acquired, plan.minecraft.client.artifact.id)?,
        &minecraft_jar,
        &operation.cancellation_token(),
    )?;

    let mut copied = HashSet::new();
    for artifact in &plan.preparation.input_artifacts {
        materialize_processor_artifact(artifact, acquired, &library_dir, &mut copied, operation)?;
    }

    for processor in &plan.preparation.processors {
        materialize_processor_artifact(
            &processor.executable_jar,
            acquired,
            &library_dir,
            &mut copied,
            operation,
        )?;

        for artifact in &processor.classpath {
            materialize_processor_artifact(
                artifact,
                acquired,
                &library_dir,
                &mut copied,
                operation,
            )?;
        }
    }

    operation.set_stage("extract-loader-inputs")?;
    let input_total = plan.preparation.embedded_inputs.len() as u64;
    operation.set_progress(Progress::Items {
        completed: 0,
        total: Some(input_total),
    })?;

    let mut embedded_input_sha256 = BTreeMap::new();
    for (index, input) in plan.preparation.embedded_inputs.iter().enumerate() {
        checkpoint(operation)?;
        let relative = minecraft_path_to_platform(&input.staging_path)?;
        if !input.staging_path.as_str().starts_with("inputs/") {
            return Err(install_error(
                ErrorCode::LoaderInstallerInvalid,
                "loader embedded input staging destination is outside the input root",
            ));
        }

        let destination = relative.under(work_root);
        let extraction_destination = destination.clone();
        let entry = input.entry.clone();
        let archive_path = installer_path.clone();
        let cancellation = operation.cancellation_token();
        let maximum_size = input.maximum_size;

        spawn_blocking_install(move || {
            archive::extract_selected_entry(
                &archive_path,
                &entry,
                &extraction_destination,
                maximum_size,
                &cancellation,
            )
        })
        .await?;

        let digest = locally_derived_sha256(&destination, &operation.cancellation_token())?;
        embedded_input_sha256.insert(input.entry.clone(), digest);
        operation.set_progress(Progress::Items {
            completed: index as u64 + 1,
            total: Some(input_total),
        })?;
    }

    let outputs_by_id = plan
        .preparation
        .generated_outputs
        .iter()
        .map(|output| (output.id.as_str(), output))
        .collect::<BTreeMap<_, _>>();
    operation.set_stage("run-loader-processors")?;

    let processor_total = plan.preparation.processors.len() as u64;
    operation.set_progress(Progress::Items {
        completed: 0,
        total: Some(processor_total),
    })?;

    for (index, processor) in plan.preparation.processors.iter().enumerate() {
        checkpoint(operation)?;
        if !processor.side.executes_for_client() {
            operation.set_progress(Progress::Items {
                completed: index as u64 + 1,
                total: Some(processor_total),
            })?;
            continue;
        }

        let declared = processor
            .declared_outputs
            .iter()
            .map(|id| {
                outputs_by_id.get(id.as_str()).copied().ok_or_else(|| {
                    install_error(
                        ErrorCode::InstallPlanInvalid,
                        "loader processor references an unknown generated output",
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let mut all_reusable = !declared.is_empty();
        for output in &declared {
            if !reusable_generated_output(
                data_root,
                output,
                component,
                &embedded_input_sha256,
                &operation.cancellation_token(),
            )? {
                all_reusable = false;
                break;
            }
        }

        if all_reusable {
            for output in &declared {
                materialize_reused_output(
                    data_root,
                    work_root,
                    output,
                    &operation.cancellation_token(),
                )?;
            }
            operation.set_progress(Progress::Items {
                completed: index as u64 + 1,
                total: Some(processor_total),
            })?;
            continue;
        }

        let executable_jar = processor_artifact_path(&processor.executable_jar, &library_dir)?;
        let classpath = processor
            .classpath
            .iter()
            .map(|artifact| processor_artifact_path(artifact, &library_dir))
            .collect::<Result<Vec<_>>>()?;
        let expansion = ProcessorExpansionContext {
            staging_root: work_root,
            root: &root,
            installer: &installer_path,
            library_dir: &library_dir,
            minecraft_jar: &minecraft_jar,
            minecraft_version,
            data: &plan.preparation.data,
        };
        let arguments = expand_processor_arguments(&processor.arguments, &expansion)?;
        let main_class = match &processor.main_class {
            Some(value) => value.clone(),
            None => {
                archive::processor_main_class(&executable_jar, &operation.cancellation_token())?
            }
        };
        let java_requirement = processor
            .java_requirement
            .as_ref()
            .or(plan.preparation.installer_java_requirement.as_ref());
        let java = runner
            .select_java(java_requirement, operation.cancellation_token())
            .await?;
        let run = runner
            .run(ToolRunRequest {
                processor_id: processor.id.clone(),
                java,
                executable_jar,
                classpath,
                main_class,
                arguments,
                working_directory: root.clone(),
                timeout: Duration::from_secs(processor.timeout_seconds),
                cancellation: operation.cancellation_token(),
            })
            .await?;

        if run.exit_code != Some(0) {
            return Err(install_error(
                ErrorCode::LoaderProcessorFailed,
                "loader processor exited unsuccessfully",
            )
            .with_context("processor", processor.id.clone())
            .with_context(
                "exit_code",
                run.exit_code
                    .map_or_else(|| "terminated".to_owned(), |value| value.to_string()),
            ));
        }

        operation.set_stage("verify-loader-outputs")?;

        for output in declared {
            checkpoint(operation)?;
            verify_and_publish_generated_output(
                data_root,
                work_root,
                output,
                component,
                &embedded_input_sha256,
                &operation.cancellation_token(),
            )?;
        }

        operation.set_stage("run-loader-processors")?;
        operation.set_progress(Progress::Items {
            completed: index as u64 + 1,
            total: Some(processor_total),
        })?;
    }

    for output in &plan.preparation.generated_outputs {
        if matches!(output.scope, GeneratedOutputScope::SharedImmutable)
            && !reusable_generated_output(
                data_root,
                output,
                component,
                &embedded_input_sha256,
                &operation.cancellation_token(),
            )?
        {
            return Err(install_error(
                ErrorCode::LoaderProcessorOutputMissing,
                "loader preparation did not produce every declared shared output",
            )
            .with_context("output", output.id.clone()));
        }
    }

    Ok(())
}

fn materialize_reused_output(
    data_root: &DataRoot,
    work_root: &Path,
    output: &graphene_minecraft::GeneratedOutput,
    cancellation: &CancellationToken,
) -> Result<()> {
    if !matches!(output.scope, GeneratedOutputScope::SharedImmutable) {
        return Ok(());
    }

    let source = minecraft_path_to_platform(&output.managed_destination)?.under(data_root.path());
    let destination = minecraft_path_to_platform(&output.staging_path)?.under(work_root);

    if destination.exists() {
        return Ok(());
    }

    copy_private(&source, &destination, cancellation)
}

fn materialize_processor_artifact(
    artifact: &ResolvedArtifact,
    acquired: &HashMap<ArtifactId, AcquiredArtifact>,
    library_dir: &Path,
    copied: &mut HashSet<PathBuf>,
    operation: &OperationController,
) -> Result<()> {
    let destination = processor_artifact_path(artifact, library_dir)?;
    if !copied.insert(destination.clone()) {
        return Ok(());
    }

    copy_private(
        acquired_source(acquired, artifact.artifact.id)?,
        &destination,
        &operation.cancellation_token(),
    )
}

fn processor_artifact_path(artifact: &ResolvedArtifact, library_dir: &Path) -> Result<PathBuf> {
    let relative = artifact
        .relative_path
        .as_str()
        .strip_prefix("shared/libraries/")
        .ok_or_else(|| {
            install_error(
                ErrorCode::LoaderProcessorUnsupported,
                "loader processor dependency is outside the managed library root",
            )
        })?;
    let relative = ManagedRelativePath::new(relative)?;
    Ok(relative.under(library_dir))
}

fn copy_private(source: &Path, destination: &Path, cancellation: &CancellationToken) -> Result<()> {
    checkpoint_token(cancellation)?;
    let metadata = fs::symlink_metadata(source).map_err(|source| {
        install_error(
            ErrorCode::InstallStageFailed,
            "failed to inspect verified loader input",
        )
        .with_source(source)
    })?;

    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(install_error(
            ErrorCode::InstallStageFailed,
            "verified loader input is not an ordinary file",
        ));
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(stage_error)?;
    }
    let mut input = fs::File::open(source).map_err(stage_error)?;
    let mut output = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(stage_error)?;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];

    loop {
        checkpoint_token(cancellation)?;
        let read = input.read(&mut buffer).map_err(stage_error)?;
        if read == 0 {
            break;
        }

        output.write_all(&buffer[..read]).map_err(stage_error)?;
    }

    output.sync_all().map_err(stage_error)?;

    Ok(())
}

fn checkpoint(operation: &OperationController) -> Result<()> {
    if operation.is_cancelled() {
        Err(cancelled_error())
    } else {
        Ok(())
    }
}

fn checkpoint_token(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        Err(cancelled_error())
    } else {
        Ok(())
    }
}

fn stage_error(source: std::io::Error) -> graphene_core::GrapheneError {
    install_error(
        ErrorCode::InstallStageFailed,
        "loader preparation staging I/O failed",
    )
    .with_source(source)
}
