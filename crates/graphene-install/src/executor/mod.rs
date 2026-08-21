mod lock;
mod materialize;
mod metadata;
mod preparation;
mod staging;

use self::{
    lock::{InstanceInstallLock, reject_existing_target},
    materialize::{
        MaterializedIntegrity, acquired_entry, acquired_source, materialize_file, validate_acquired,
    },
    metadata::write_instance_metadata,
    preparation::execute_preparation,
    staging::{remove_tree_blocking, validate_staging},
};
use crate::{
    acquisition::{AcquiredArtifact, ArtifactAcquirer},
    archive,
    error::{cancelled_error, install_error},
    path::{minecraft_path_to_platform, receipt_path_to_platform},
    plan::InstallPlan,
    processor::InstallToolRunner,
};
use graphene_core::{ArtifactId, ErrorCode, OperationController, Progress, Result};
use graphene_instance::CommittedInstance;
use graphene_platform::{ManagedRelativePath, publish_directory_create_only};
use graphene_storage::DataRoot;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::ErrorKind as IoErrorKind,
    path::Path,
    sync::Arc,
};

#[derive(Clone)]
pub struct InstallExecutor {
    data_root: DataRoot,
    acquirer: Arc<dyn ArtifactAcquirer>,
    tool_runner: Option<Arc<dyn InstallToolRunner>>,
}

impl std::fmt::Debug for InstallExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallExecutor")
            .field("data_root", &self.data_root.path())
            .finish_non_exhaustive()
    }
}

impl InstallExecutor {
    pub fn new(data_root: DataRoot, acquirer: Arc<dyn ArtifactAcquirer>) -> Self {
        Self {
            data_root,
            acquirer,
            tool_runner: None,
        }
    }

    #[must_use]
    pub fn with_tool_runner(mut self, runner: Arc<dyn InstallToolRunner>) -> Self {
        self.tool_runner = Some(runner);
        self
    }

    /// Executes with one in-flight acquisition at a time. This intentionally conservative
    /// scheduler is bounded and reuses the service's own download concurrency when acquisitions are
    /// composed with other operations.
    pub async fn execute(
        &self,
        plan: InstallPlan,
        operation: &OperationController,
    ) -> Result<CommittedInstance> {
        plan.validate()?;
        let final_root = self
            .data_root
            .path()
            .join(plan.instance.relative_root.as_str());
        reject_existing_target(&final_root)?;
        let lock = InstanceInstallLock::acquire(
            self.data_root.path(),
            plan.instance.descriptor.instance_id,
        )?;
        reject_existing_target(&final_root)?;

        let staging_relative = ManagedRelativePath::new(format!(
            "instances/.staging/{}-{}",
            plan.instance.descriptor.instance_id,
            operation.handle().id()
        ))?;
        let staging_root = staging_relative.under(self.data_root.path());
        if staging_root.exists() {
            remove_tree_blocking(staging_root.clone()).await?;
        }

        let result = self
            .execute_inner(&plan, operation, &staging_root, &final_root)
            .await;
        if result.is_err() && staging_root.exists() {
            let cleanup = remove_tree_blocking(staging_root.clone()).await;
            if cleanup.is_err() {
                // Keep the original structured failure. Staging paths are deliberately recognizable
                // and are never interpreted as committed instances.
            }
        }

        drop(lock);
        result
    }

    async fn execute_inner(
        &self,
        plan: &InstallPlan,
        operation: &OperationController,
        staging_root: &Path,
        final_root: &Path,
    ) -> Result<CommittedInstance> {
        checkpoint(operation)?;
        operation.set_stage("acquire-artifacts")?;
        let artifact_total = plan.artifacts.len() as u64;
        operation.set_progress(Progress::Items {
            completed: 0,
            total: Some(artifact_total),
        })?;

        let mut acquired = HashMap::<ArtifactId, AcquiredArtifact>::new();
        for (index, planned) in plan.artifacts.iter().enumerate() {
            checkpoint(operation)?;
            let result = self
                .acquirer
                .acquire(planned.artifact.clone(), operation.handle())
                .await?;
            validate_acquired(&planned.artifact, &result)?;
            acquired.insert(planned.artifact.id, result);
            operation.set_progress(Progress::Items {
                completed: index as u64 + 1,
                total: Some(artifact_total),
            })?;
        }

        checkpoint(operation)?;
        operation.set_stage("materialize-shared")?;
        let shared_total = plan.shared_materializations.len() as u64;
        operation.set_progress(Progress::Items {
            completed: 0,
            total: Some(shared_total),
        })?;

        for (index, materialization) in plan.shared_materializations.iter().enumerate() {
            checkpoint(operation)?;
            let acquired_artifact = acquired_entry(&acquired, materialization.artifact_id)?;
            let root = self.data_root.path().to_path_buf();
            let relative = minecraft_path_to_platform(&materialization.destination)?;
            let source = acquired_artifact.path.clone();
            let expected = MaterializedIntegrity {
                bytes: acquired_artifact.bytes,
                sha1: acquired_artifact.sha1,
                sha256: acquired_artifact.sha256,
            };
            let cancellation = operation.handle().cancellation_token();
            spawn_blocking_install(move || {
                materialize_file(&source, &root, &relative, Some(expected), &cancellation)
            })
            .await?;

            operation.set_progress(Progress::Items {
                completed: index as u64 + 1,
                total: Some(shared_total),
            })?;
        }

        checkpoint(operation)?;
        operation.set_stage("stage-instance")?;
        let staging_parent = staging_root.parent().ok_or_else(|| {
            install_error(
                ErrorCode::InstallStageFailed,
                "staging target has no parent",
            )
        })?;

        fs::create_dir_all(staging_parent).map_err(|source| {
            install_error(
                ErrorCode::InstallStageFailed,
                "failed to create instance staging root",
            )
            .with_source(source)
        })?;

        fs::create_dir(staging_root).map_err(|source| {
            install_error(
                ErrorCode::InstallStageFailed,
                "failed to create isolated instance staging directory",
            )
            .with_source(source)
        })?;

        let instance_total = plan.instance_materializations.len() as u64;
        operation.set_progress(Progress::Items {
            completed: 0,
            total: Some(instance_total),
        })?;
        for (index, materialization) in plan.instance_materializations.iter().enumerate() {
            checkpoint(operation)?;
            let source = acquired_source(&acquired, materialization.artifact_id)?;
            let relative = minecraft_path_to_platform(&materialization.destination)?;
            let source = source.to_path_buf();
            let root = staging_root.to_path_buf();
            let cancellation = operation.handle().cancellation_token();
            spawn_blocking_install(move || {
                materialize_file(&source, &root, &relative, None, &cancellation)
            })
            .await?;
            operation.set_progress(Progress::Items {
                completed: index as u64 + 1,
                total: Some(instance_total),
            })?;
        }

        checkpoint(operation)?;
        execute_preparation(
            &self.data_root,
            plan,
            &acquired,
            self.tool_runner.as_ref(),
            operation,
            staging_root,
        )
        .await?;

        checkpoint(operation)?;
        operation.set_stage("extract-natives")?;
        let native_relative = receipt_path_to_platform(&plan.receipt.natives_directory)?;
        let native_root = native_relative.under(staging_root);
        let native_parent = native_root.parent().ok_or_else(|| {
            install_error(
                ErrorCode::InstallNativeExtractionFailed,
                "native target has no parent",
            )
        })?;

        fs::create_dir_all(native_parent).map_err(|source| {
            install_error(
                ErrorCode::InstallNativeExtractionFailed,
                "failed to create native staging parent",
            )
            .with_source(source)
        })?;

        fs::create_dir(&native_root).map_err(|source| {
            install_error(
                ErrorCode::InstallNativeExtractionFailed,
                "failed to create native staging directory",
            )
            .with_source(source)
        })?;

        let native_total = plan.native_extractions.len() as u64;
        operation.set_progress(Progress::Items {
            completed: 0,
            total: Some(native_total),
        })?;
        let mut created_native_roots =
            HashSet::<String>::from([plan.receipt.natives_directory.as_str().to_owned()]);
        for (index, extraction) in plan.native_extractions.iter().enumerate() {
            checkpoint(operation)?;
            let source = acquired_source(&acquired, extraction.artifact_id)?.to_path_buf();
            let relative = minecraft_path_to_platform(&extraction.destination)?;
            let destination = relative.under(staging_root);
            if created_native_roots.insert(extraction.destination.as_str().to_owned()) {
                let parent = destination.parent().ok_or_else(|| {
                    install_error(
                        ErrorCode::InstallNativeExtractionFailed,
                        "native target has no parent",
                    )
                })?;

                fs::create_dir_all(parent).map_err(|source| {
                    install_error(
                        ErrorCode::InstallNativeExtractionFailed,
                        "failed to create native staging parent",
                    )
                    .with_source(source)
                })?;

                fs::create_dir(&destination).map_err(|source| {
                    install_error(
                        ErrorCode::InstallNativeExtractionFailed,
                        "failed to create native staging directory",
                    )
                    .with_source(source)
                })?;
            }

            let cancellation = operation.handle().cancellation_token();
            spawn_blocking_install(move || {
                archive::extract_native_zip(&source, &destination, &cancellation)
            })
            .await?;
            operation.set_progress(Progress::Items {
                completed: index as u64 + 1,
                total: Some(native_total),
            })?;
        }

        checkpoint(operation)?;
        operation.set_stage("write-metadata")?;
        write_instance_metadata(staging_root, &plan.instance.descriptor, &plan.receipt).await?;

        checkpoint(operation)?;
        operation.set_stage("validate-staging")?;
        // Yield after publishing the stage so cancellation and host-side diagnostics can observe
        // the boundary before validation begins. This does not weaken the transaction: the tree is
        // still isolated staging state and publication has not been sealed.
        tokio::task::yield_now().await;
        checkpoint(operation)?;
        validate_staging(self.data_root.path(), staging_root, plan).await?;

        operation.set_stage("pre-commit")?;
        tokio::task::yield_now().await;
        checkpoint(operation)?;
        reject_existing_target(final_root)?;
        if !operation.seal_cancellation() {
            return Err(cancelled_error());
        }

        operation.set_stage("commit")?;
        let staging = staging_root.to_path_buf();
        let final_path = final_root.to_path_buf();
        spawn_blocking_install(move || {
            reject_existing_target(&final_path)?;
            publish_directory_create_only(&staging, &final_path).map_err(|source| {
                let code = if source.kind() == IoErrorKind::AlreadyExists {
                    ErrorCode::InstallTargetExists
                } else {
                    ErrorCode::InstallCommitFailed
                };

                install_error(code, "failed to publish the staged instance").with_source(source)
            })
        })
        .await?;

        Ok(CommittedInstance {
            descriptor: plan.instance.descriptor.clone(),
        })
    }
}

pub(super) async fn spawn_blocking_install<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work).await.map_err(|source| {
        install_error(
            ErrorCode::InstallStageFailed,
            "blocking installation worker failed",
        )
        .with_source(source)
    })?
}

fn checkpoint(operation: &OperationController) -> Result<()> {
    if operation.is_cancelled() {
        Err(cancelled_error())
    } else {
        Ok(())
    }
}
