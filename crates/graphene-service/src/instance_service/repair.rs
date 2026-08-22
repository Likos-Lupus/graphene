use crate::{
    ArtifactService,
    context::ServiceContext,
    instance_service::{repository::InstanceRepository, verification::execute_verify},
    operation_lifecycle,
};
use graphene_core::{
    Artifact, ArtifactId, ErrorCode, ErrorKind, GrapheneError, InstanceId, OperationController,
    OperationHandle, Progress, Result,
};
use graphene_instance::{
    FindingSeverity, InstanceLockfile, InstanceStateFingerprint, LockedMaterializationScope,
    REPAIR_PLAN_SCHEMA_VERSION, RepairAction, RepairOptions, RepairPlan, RepairResult,
};
use graphene_platform::replace_file_safely;
use graphene_storage::{MAX_LOCKFILE_BYTES, read_document_bounded};
use std::{collections::HashSet, fs, future::Future, pin::Pin, sync::Arc};

/// Prepared asynchronous instance repair operation handle.
pub struct InstanceRepairOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = Result<RepairResult>> + Send>>,
}

impl InstanceRepairOperation {
    /// Returns the public operation handle for progress observation and cancellation.
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }

    /// Awaits completion of the repair transaction.
    pub async fn await_result(self) -> Result<RepairResult> {
        self.future.await
    }
}

impl std::fmt::Debug for InstanceRepairOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceRepairOperation")
            .field("operation_id", &self.operation.id())
            .finish_non_exhaustive()
    }
}

/// Constructs a deterministic, non-mutating repair plan from durable desired state.
pub async fn plan_repair(
    context: Arc<ServiceContext>,
    instance_id: InstanceId,
    options: RepairOptions,
) -> Result<RepairPlan> {
    let verify_controller = context.operations.create("repair-preverify");
    operation_lifecycle::start(&verify_controller)?;
    let verify_report = execute_verify(
        Arc::clone(&context),
        instance_id,
        options.verification_mode,
        &verify_controller,
    )
    .await?;
    let _ = verify_controller.succeed();

    let repo = InstanceRepository::new(context.storage.path());
    let _lease = repo.acquire_shared_lease(instance_id)?;

    let receipt = repo.load_receipt(instance_id)?;
    let lockfile_path = repo.paths().lockfile_path(instance_id);
    let lockfile = if lockfile_path.exists() {
        let bytes = read_document_bounded(&lockfile_path, MAX_LOCKFILE_BYTES)?;
        serde_json::from_slice::<InstanceLockfile>(&bytes).ok()
    } else {
        None
    };
    let effective_config = repo.effective_config(instance_id).ok();
    let base_fingerprint = InstanceStateFingerprint::compute(
        instance_id,
        &receipt,
        lockfile.as_ref(),
        effective_config.as_ref(),
    );

    if verify_report.is_healthy() {
        return Ok(RepairPlan {
            schema_version: REPAIR_PLAN_SCHEMA_VERSION,
            instance_id,
            base_state_fingerprint: base_fingerprint,
            verification_mode: options.verification_mode,
            ordered_actions: Vec::new(),
            estimated_download_bytes: 0,
            requires_network: false,
            requires_tool_java: false,
            residual_diagnostics: Vec::new(),
        });
    }

    let mut acquire_actions = Vec::new();
    let mut restore_shared_actions = Vec::new();
    let mut restore_instance_actions = Vec::new();
    let mut prep_actions = Vec::new();
    let mut residual_diagnostics = Vec::new();

    let mut seen_keys = HashSet::new();
    let mut estimated_download_bytes: u64 = 0;
    let mut requires_network = false;

    if let Some(lock) = &lockfile {
        for finding in &verify_report.findings {
            if finding.severity != FindingSeverity::Error {
                continue;
            }

            if !finding.repairable {
                residual_diagnostics.push(finding.clone());
                continue;
            }

            if let Some(rel_path) = &finding.path {
                if let Some(art) = lock
                    .artifacts
                    .iter()
                    .find(|a| a.destination.as_str() == rel_path.as_str())
                {
                    if seen_keys.insert(art.logical_key.clone()) {
                        acquire_actions.push(RepairAction::AcquireArtifact {
                            logical_key: art.logical_key.clone(),
                            destination: art.destination.clone(),
                        });
                        if !art.sources.is_empty() {
                            requires_network = true;
                            if let Some(size) = art.expected_size {
                                estimated_download_bytes += size;
                            }
                        }
                    }

                    match art.scope {
                        LockedMaterializationScope::SharedImmutable => {
                            restore_shared_actions.push(
                                RepairAction::RestoreSharedMaterialization {
                                    destination: art.destination.clone(),
                                },
                            );
                        }
                        LockedMaterializationScope::InstanceMutable => {
                            restore_instance_actions.push(
                                RepairAction::RestoreInstanceMaterialization {
                                    destination: art.destination.clone(),
                                },
                            );
                        }
                    }
                } else if let Some(generated) = lock
                    .generated_outputs
                    .iter()
                    .find(|g| g.destination.as_str() == rel_path.as_str())
                {
                    prep_actions.push(RepairAction::RunPreparation {
                        component_uid: generated.component_uid.clone(),
                    });
                    restore_shared_actions.push(RepairAction::RestoreGeneratedOutput {
                        destination: generated.destination.clone(),
                    });
                } else {
                    residual_diagnostics.push(finding.clone());
                }
            } else {
                residual_diagnostics.push(finding.clone());
            }
        }
    } else {
        // Legacy instance without lockfile
        residual_diagnostics.extend(verify_report.findings.iter().cloned());
    }

    let mut ordered_actions = Vec::new();
    ordered_actions.extend(acquire_actions);
    ordered_actions.extend(restore_shared_actions);
    ordered_actions.extend(restore_instance_actions);
    let requires_tool_java = !prep_actions.is_empty();
    ordered_actions.extend(prep_actions);

    Ok(RepairPlan {
        schema_version: REPAIR_PLAN_SCHEMA_VERSION,
        instance_id,
        base_state_fingerprint: base_fingerprint,
        verification_mode: options.verification_mode,
        ordered_actions,
        estimated_download_bytes,
        requires_network,
        requires_tool_java,
        residual_diagnostics,
    })
}

pub(crate) fn start_repair_operation(
    context: Arc<ServiceContext>,
    plan: RepairPlan,
) -> InstanceRepairOperation {
    let controller = context.operations.create("instance-repair");
    let operation = controller.handle();
    let future = Box::pin(async move {
        operation_lifecycle::start(&controller)?;
        let result = execute_repair(context, plan, &controller).await;
        operation_lifecycle::finish(
            &controller,
            result,
            ErrorCode::OperationCancelled,
            ErrorKind::Cancelled,
            "instance repair operation was cancelled",
        )
    });

    InstanceRepairOperation { operation, future }
}

async fn execute_repair(
    context: Arc<ServiceContext>,
    plan: RepairPlan,
    controller: &OperationController,
) -> Result<RepairResult> {
    controller.set_stage("validate-plan")?;
    let repo = InstanceRepository::new(context.storage.path());
    let _lease = repo.acquire_exclusive_lease(plan.instance_id)?;

    // 1. Stale plan validation: check current state fingerprint against plan base
    let receipt = repo.load_receipt(plan.instance_id)?;
    let lockfile_path = repo.paths().lockfile_path(plan.instance_id);
    let lockfile = if lockfile_path.exists() {
        let bytes = read_document_bounded(&lockfile_path, MAX_LOCKFILE_BYTES)?;
        serde_json::from_slice::<InstanceLockfile>(&bytes).ok()
    } else {
        None
    };
    let effective_config = repo.effective_config(plan.instance_id).ok();
    let current_fingerprint = InstanceStateFingerprint::compute(
        plan.instance_id,
        &receipt,
        lockfile.as_ref(),
        effective_config.as_ref(),
    );

    if current_fingerprint != plan.base_state_fingerprint {
        return Err(GrapheneError::new(
            ErrorCode::InstanceRepairPlanStale,
            ErrorKind::Instance,
            "instance state changed between repair planning and execution",
        )
        .with_context("instance_id", plan.instance_id.to_string()));
    }

    if plan.is_noop() {
        let post_verify_report = execute_verify(
            Arc::clone(&context),
            plan.instance_id,
            plan.verification_mode,
            controller,
        )
        .await?;
        return Ok(RepairResult {
            instance_id: plan.instance_id,
            executed_actions_count: 0,
            post_verify_report,
        });
    }

    let lock = lockfile.ok_or_else(|| {
        GrapheneError::new(
            ErrorCode::InstanceRepairFailed,
            ErrorKind::Instance,
            "cannot execute repair on an instance without a desired-state lockfile",
        )
    })?;

    let artifact_service = ArtifactService::new(Arc::clone(&context));
    let total_actions = plan.ordered_actions.len();
    let mut executed_count = 0;

    controller.set_stage("execute-repair-actions")?;

    for action in &plan.ordered_actions {
        match action {
            RepairAction::AcquireArtifact { destination, .. } => {
                if let Some(art) = lock
                    .artifacts
                    .iter()
                    .find(|a| a.destination.as_str() == destination.as_str())
                {
                    let mut core_artifact =
                        Artifact::new(art.sources.clone(), art.integrity.clone());
                    core_artifact.id = ArtifactId::new();
                    core_artifact.expected_size = art.expected_size;

                    let verified = artifact_service
                        .acquire(core_artifact, Some(&controller.handle()))
                        .await_result()
                        .await?;

                    // Determine destination path on disk
                    let dest_path = match art.scope {
                        LockedMaterializationScope::SharedImmutable => {
                            context.storage.path().join(art.destination.as_str())
                        }
                        LockedMaterializationScope::InstanceMutable => repo
                            .paths()
                            .instance_root(plan.instance_id)
                            .join(art.destination.as_str()),
                    };

                    if let Some(parent) = dest_path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }

                    // Copy from verified cache path into destination atomically
                    let tmp = dest_path.with_extension("graphene-repair-part");
                    fs::copy(&verified.path, &tmp).map_err(|source| {
                        GrapheneError::new(
                            ErrorCode::FileWriteFailed,
                            ErrorKind::Filesystem,
                            "failed to stage repaired artifact",
                        )
                        .with_source(source)
                    })?;
                    replace_file_safely(&tmp, &dest_path)?;
                }
            }
            RepairAction::RestoreSharedMaterialization { destination }
            | RepairAction::RestoreInstanceMaterialization { destination } => {
                if let Some(art) = lock
                    .artifacts
                    .iter()
                    .find(|a| a.destination.as_str() == destination.as_str())
                {
                    let cache_addr = context.storage.cache_address(&art.integrity)?;
                    if context.storage.committed_file_exists(cache_addr.path())? {
                        let dest_path = match art.scope {
                            LockedMaterializationScope::SharedImmutable => {
                                context.storage.path().join(art.destination.as_str())
                            }
                            LockedMaterializationScope::InstanceMutable => repo
                                .paths()
                                .instance_root(plan.instance_id)
                                .join(art.destination.as_str()),
                        };
                        if let Some(parent) = dest_path.parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        let tmp = dest_path.with_extension("graphene-repair-part");
                        fs::copy(cache_addr.path(), &tmp).map_err(|source| {
                            GrapheneError::new(
                                ErrorCode::FileWriteFailed,
                                ErrorKind::Filesystem,
                                "failed to stage repaired materialization from cache",
                            )
                            .with_source(source)
                        })?;
                        replace_file_safely(&tmp, &dest_path)?;
                    }
                }
            }
            _ => {}
        }

        executed_count += 1;
        let _ = controller.set_progress(Progress::Items {
            completed: executed_count as u64,
            total: Some(total_actions as u64),
        });
    }

    controller.set_stage("post-repair-verify")?;
    drop(_lease);
    let post_verify_report = execute_verify(
        Arc::clone(&context),
        plan.instance_id,
        plan.verification_mode,
        controller,
    )
    .await?;

    Ok(RepairResult {
        instance_id: plan.instance_id,
        executed_actions_count: executed_count,
        post_verify_report,
    })
}
