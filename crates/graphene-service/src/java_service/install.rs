use super::{
    JavaService,
    support::{cancelled, java_error, spawn_blocking_java},
};
use crate::ArtifactService;
use graphene_core::{ErrorCode, OperationController, Result};
use graphene_install::{extract_managed_tar_gz, extract_managed_zip};
use graphene_java::{
    JavaArchitecture, JavaCandidate, JavaCandidateSource, JavaRequirement, JavaRuntime,
    MANAGED_RUNTIME_SCHEMA_VERSION, ManagedArchiveFormat, ManagedJavaInstallPlan,
    ManagedJavaRequest, ManagedJavaRuntime, is_compatible, probe_java,
};
use graphene_storage::ManagedRuntimeStore;
use std::{fs, path::PathBuf, sync::Arc};

use super::inventory::{
    locate_java_executable, make_executable, runtime_dir_exists, validate_and_probe_committed,
};

impl JavaService {
    pub(super) async fn install_managed_inner(
        &self,
        requirement: JavaRequirement,
        controller: &OperationController,
    ) -> Result<JavaRuntime> {
        if controller.is_cancelled() {
            return Err(cancelled());
        }

        controller.set_stage("resolve_managed_release")?;
        let request = ManagedJavaRequest::for_current_platform(&requirement)?;
        let release = self
            .context
            .java_distribution_provider
            .resolve_release(&request, controller)
            .await?;
        let plan = ManagedJavaInstallPlan::new(request, release)?;
        let runtime_id = plan.release.runtime_id;
        let gate = self.context.runtime_gate(runtime_id);
        let token = controller.cancellation_token();
        let _guard = tokio::select! { () = token.cancelled() => return Err(cancelled()), guard = gate.lock() => guard };

        let runtime_dir = self
            .context
            .storage
            .path()
            .join("shared/runtimes")
            .join(runtime_id.to_string());
        if let Some(existing) = self.load_one_runtime(runtime_id).await? {
            return validate_and_probe_committed(existing, runtime_dir, &requirement).await;
        }

        if runtime_dir_exists(runtime_dir.clone()).await? {
            return Err(java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "managed runtime directory exists without a valid descriptor",
            ));
        }

        controller.set_stage("download_archive")?;
        let acquired = ArtifactService::new(Arc::clone(&self.context))
            .acquire(plan.release.artifact.clone(), Some(&controller.handle()))
            .await_result()
            .await?;
        controller.set_stage("verify_archive")?;
        if acquired.sha256 != plan.release.archive_sha256 {
            return Err(java_error(
                ErrorCode::HashMismatch,
                "managed Java artifact checksum does not match release plan",
            ));
        }

        if controller.is_cancelled() {
            return Err(cancelled());
        }

        controller.set_stage("extract_runtime")?;
        let root = self.context.storage.clone();
        let operation_id = controller.handle().id();
        let archive = acquired.path.clone();
        let format = plan.release.archive_format;
        let cancellation = controller.cancellation_token();
        let staging = spawn_blocking_java(move || {
            let store = ManagedRuntimeStore::new(&root)?;
            let staging = store.prepare_staging(operation_id)?;
            let payload = match store.payload_dir(operation_id) {
                Ok(path) => path,
                Err(error) => {
                    let _ = store.cleanup_staging(operation_id);
                    return Err(error);
                }
            };

            let extracted = match format {
                ManagedArchiveFormat::Zip => extract_managed_zip(&archive, &payload, &cancellation),
                ManagedArchiveFormat::TarGz => {
                    extract_managed_tar_gz(&archive, &payload, &cancellation)
                }
                _ => Err(java_error(
                    ErrorCode::JavaManagedArchiveInvalid,
                    "managed Java install plan uses an unsupported archive format",
                )),
            };

            if let Err(source) = extracted {
                let _ = store.cleanup_staging(operation_id);
                if source.is_cancelled() {
                    return Err(cancelled());
                }
                return Err(java_error(
                    ErrorCode::JavaManagedArchiveInvalid,
                    "managed Java archive extraction failed",
                )
                .with_source(source));
            }
            Ok(staging)
        })
        .await?;

        let result = self
            .finish_staged_runtime(&plan, &requirement, controller, staging)
            .await;
        if result.is_err() {
            let root = self.context.storage.clone();
            let _ = spawn_blocking_java(move || {
                ManagedRuntimeStore::new(&root)?.cleanup_staging(operation_id)
            })
            .await;
        }
        result
    }

    async fn finish_staged_runtime(
        &self,
        plan: &ManagedJavaInstallPlan,
        requirement: &JavaRequirement,
        controller: &OperationController,
        staging: PathBuf,
    ) -> Result<JavaRuntime> {
        if controller.is_cancelled() {
            return Err(cancelled());
        }

        controller.set_stage("probe_runtime")?;
        let search_root = staging.join("runtime");
        let candidate_path =
            spawn_blocking_java(move || locate_java_executable(&search_root)).await?;
        make_executable(&candidate_path).await?;
        let candidate = JavaCandidate {
            executable: candidate_path.clone(),
            source: JavaCandidateSource::Managed,
        };
        let probed = probe_java(&candidate).await.map_err(|source| {
            java_error(
                ErrorCode::JavaRuntimeUnexecutable,
                "staged managed Java executable failed bounded probe",
            )
            .with_source(source)
        })?;

        if !is_compatible(&probed, requirement, JavaArchitecture::current())
            || probed.architecture != plan.release.architecture
        {
            let code = if probed.architecture != plan.release.architecture {
                ErrorCode::JavaArchitectureMismatch
            } else if probed.major_version < requirement.major_version {
                ErrorCode::JavaVersionTooOld
            } else if probed.major_version > requirement.major_version {
                ErrorCode::JavaVersionTooNew
            } else {
                ErrorCode::JavaIncompatible
            };
            return Err(java_error(
                code,
                "staged managed Java runtime does not satisfy the requested compatibility",
            ));
        }

        let relative = candidate_path.strip_prefix(&staging).map_err(|_| {
            java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "staged Java executable escaped runtime staging",
            )
        })?;
        let executable_relative_path = relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let descriptor = ManagedJavaRuntime {
            schema_version: MANAGED_RUNTIME_SCHEMA_VERSION,
            id: plan.release.runtime_id,
            provider: plan.release.provider.clone(),
            distribution: plan.release.distribution.clone(),
            release_name: plan.release.release_name.clone(),
            version: probed.version.clone(),
            major_version: probed.major_version,
            vendor: probed.vendor.clone(),
            os: plan.release.os,
            architecture: probed.architecture,
            image: plan.release.image,
            archive_sha256: plan.release.archive_sha256,
            executable_relative_path,
        };
        descriptor.validate()?;
        controller.set_stage("commit")?;
        let bytes = serde_json::to_vec_pretty(&descriptor).map_err(|source| {
            java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "failed to serialize managed runtime descriptor",
            )
            .with_source(source)
        })?;
        let root = self.context.storage.clone();
        let operation_id = controller.handle().id();
        let descriptor_for_validation = descriptor.clone();

        spawn_blocking_java(move || {
            let store = ManagedRuntimeStore::new(&root)?;
            store.write_staging_descriptor(operation_id, &bytes)?;
            let reread = fs::read(store.staging_dir(operation_id).join("runtime.json")).map_err(
                |source| {
                    java_error(
                        ErrorCode::JavaManagedRuntimeCorrupt,
                        "failed to re-read staged runtime descriptor",
                    )
                    .with_source(source)
                },
            )?;
            let roundtrip: ManagedJavaRuntime =
                serde_json::from_slice(&reread).map_err(|source| {
                    java_error(
                        ErrorCode::JavaManagedRuntimeCorrupt,
                        "staged runtime descriptor does not round-trip",
                    )
                    .with_source(source)
                })?;
            roundtrip.validate()?;
            if roundtrip != descriptor_for_validation {
                return Err(java_error(
                    ErrorCode::JavaManagedRuntimeCorrupt,
                    "staged runtime descriptor changed before publication",
                ));
            }
            Ok(())
        })
        .await?;

        tokio::task::yield_now().await;
        if controller.is_cancelled() {
            return Err(cancelled());
        }

        if !controller.seal_cancellation() {
            return Err(cancelled());
        }

        let root = self.context.storage.clone();
        let runtime_id = descriptor.id;
        spawn_blocking_java(move || {
            let store = ManagedRuntimeStore::new(&root)?;
            match store.publish(operation_id, runtime_id) {
                Ok(()) => Ok(()),
                Err(error) if store.runtime_dir(runtime_id).is_dir() => {
                    let _ = store.cleanup_staging(operation_id);
                    Err(java_error(
                        ErrorCode::JavaManagedInstallFailed,
                        "managed runtime identity was published concurrently; retry selection",
                    )
                    .with_source(error))
                }
                Err(error) => Err(error),
            }
        })
        .await?;

        let committed = self
            .context
            .storage
            .path()
            .join("shared/runtimes")
            .join(descriptor.id.to_string());
        descriptor.to_java_runtime(&committed)
    }
}
