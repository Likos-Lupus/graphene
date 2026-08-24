use crate::{
    ArtifactService,
    content_service::transaction::{
        ContentFaultPoint, ContentMutationJournal, ContentTransactionPhase,
        JournalQuarantineMapping, JournalRenameMapping, JournalStagedMapping,
        recover_content_journal,
    },
    context::ServiceContext,
    instance_service::repository::InstanceRepository,
};
use graphene_content::{
    ContentEntryId, ContentMutationPlan, PlannedFilesystemAction,
    local::inventory::scan_local_inventory,
};
use graphene_core::{
    Artifact, ArtifactId, ArtifactKind, ErrorCode, ErrorKind, GrapheneError, OperationController,
    Result, Sha256Digest,
};
use graphene_instance::{
    InstanceLockfile, InstanceStateFingerprint, LOCKFILE_SCHEMA_VERSION, LockedArtifact,
    LockedMaterializationScope,
};
use graphene_platform::replace_file_safely;
use graphene_storage::{MAX_LOCKFILE_BYTES, read_document_bounded, write_json_atomic};
use sha2::{Digest, Sha256};
use std::{fs, sync::Arc};

/// Result of executing a content mutation plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentMutationResult {
    pub modified_entry_ids: Vec<ContentEntryId>,
    pub committed_fingerprint: InstanceStateFingerprint,
}

/// Executes a content mutation plan transactionally under the exclusive instance lease.
pub async fn execute_content_mutation(
    context: &Arc<ServiceContext>,
    plan: &ContentMutationPlan,
    fault_point: Option<ContentFaultPoint>,
    operation: &OperationController,
) -> Result<ContentMutationResult> {
    plan.validate()?;

    // 1. Pre-acquisition of exact remote artifacts into immutable cache BEFORE exclusive lease
    if !plan.artifacts_to_acquire.is_empty() {
        operation.set_stage("pre-acquire-artifacts")?;
        let artifact_service = ArtifactService::new(Arc::clone(context));

        for content_file in &plan.artifacts_to_acquire {
            let mut core_art =
                Artifact::new(content_file.sources.clone(), content_file.integrity.clone());
            core_art.id = ArtifactId::new();
            core_art.kind = ArtifactKind::Binary;
            core_art.expected_size = Some(content_file.size);

            artifact_service
                .acquire(core_art, Some(&operation.handle()))
                .await_result()
                .await?;
        }
    }

    // 2. Acquire exclusive lease on instance
    operation.set_stage("acquire-exclusive-lease")?;
    let repo = InstanceRepository::new(context.storage.path());
    let _lease = repo.acquire_exclusive_lease(plan.instance_id)?;
    let instance_root = repo.paths().instance_root(plan.instance_id);

    // 3. Recover any prior interrupted transaction journal
    let lockfile_path = repo.paths().lockfile_path(plan.instance_id);
    let current_lockfile_bytes = if lockfile_path.exists() {
        Some(read_document_bounded(&lockfile_path, MAX_LOCKFILE_BYTES)?)
    } else {
        None
    };

    recover_content_journal(&instance_root, current_lockfile_bytes.as_deref())?;

    // 4. Reload committed instance state and check for stale fingerprints
    operation.set_stage("revalidate-stale-state")?;
    let receipt = repo.load_receipt(plan.instance_id)?;
    let lockfile_bytes = if lockfile_path.exists() {
        Some(read_document_bounded(&lockfile_path, MAX_LOCKFILE_BYTES)?)
    } else {
        None
    };
    let current_lockfile = match &lockfile_bytes {
        Some(bytes) => serde_json::from_slice::<InstanceLockfile>(bytes).ok(),
        None => None,
    };
    let effective_config = repo.effective_config(plan.instance_id).ok();

    let current_state_fingerprint = InstanceStateFingerprint::compute(
        plan.instance_id,
        &receipt,
        current_lockfile.as_ref(),
        effective_config.as_ref(),
    );

    if current_state_fingerprint != plan.base_state_fingerprint {
        return Err(GrapheneError::new(
            ErrorCode::InstanceStateInvalid,
            ErrorKind::Instance,
            "content mutation plan is stale: instance state has changed since plan generation",
        ));
    }

    let mods_dir = repo.paths().minecraft_dir(plan.instance_id).join("mods");
    let current_inventory = scan_local_inventory(&mods_dir, true, &operation.cancellation_token())?;

    if current_inventory.fingerprint != plan.base_inventory_fingerprint {
        return Err(GrapheneError::new(
            ErrorCode::ContentInventoryStale,
            ErrorKind::Content,
            "content mutation plan is stale: local mod inventory has changed since plan generation",
        ));
    }

    if plan.is_noop() {
        return Ok(ContentMutationResult {
            modified_entry_ids: Vec::new(),
            committed_fingerprint: current_state_fingerprint,
        });
    }

    // 5. Staging new files
    operation.set_stage("stage-content")?;
    let staging_dir = instance_root
        .join(".graphene")
        .join("staging")
        .join("content");
    fs::create_dir_all(&staging_dir).map_err(|source| {
        GrapheneError::new(
            ErrorCode::DirectoryCreateFailed,
            ErrorKind::Filesystem,
            "failed to create content staging directory",
        )
        .with_source(source)
    })?;

    let mut staged_mappings = Vec::new();
    let mut quarantine_mappings = Vec::new();
    let mut rename_mappings = Vec::new();

    for action in &plan.filesystem_actions {
        match action {
            PlannedFilesystemAction::StageArtifact {
                artifact_logical_key: _,
                destination,
            } => {
                let filename = destination.as_str().trim_start_matches(".minecraft/mods/");
                let staged_file_path = staging_dir.join(filename);
                let content_file = plan
                    .artifacts_to_acquire
                    .iter()
                    .find(|a| a.filename == filename || destination.as_str().ends_with(&a.filename))
                    .or_else(|| plan.artifacts_to_acquire.first());

                if let Some(cfile) = content_file {
                    let cache_addr = context.storage.cache_address(&cfile.integrity)?;
                    if !context.storage.committed_file_exists(cache_addr.path())? {
                        return Err(GrapheneError::new(
                            ErrorCode::CacheIdentityUnavailable,
                            ErrorKind::Integrity,
                            format!(
                                "pre-acquired artifact for {} not found in cache",
                                cfile.file_ref
                            ),
                        ));
                    }

                    fs::copy(cache_addr.path(), &staged_file_path).map_err(|source| {
                        GrapheneError::new(
                            ErrorCode::FileWriteFailed,
                            ErrorKind::Filesystem,
                            "failed to copy artifact from cache to staging",
                        )
                        .with_source(source)
                    })?;
                }
            }
            PlannedFilesystemAction::PublishStagedArtifact {
                staged_relative,
                final_destination,
            } => {
                staged_mappings.push(JournalStagedMapping {
                    staged_path: staged_relative.clone(),
                    destination: final_destination.clone(),
                });
            }
            PlannedFilesystemAction::QuarantineFile { path } => {
                let filename = path.as_str().trim_start_matches(".minecraft/mods/");
                let q_rel = format!(".graphene/quarantine/content/{filename}");
                quarantine_mappings.push(JournalQuarantineMapping {
                    original_path: path.clone(),
                    quarantine_path: q_rel,
                });
            }
            PlannedFilesystemAction::RenameFile { from, to } => {
                rename_mappings.push(JournalRenameMapping {
                    from: from.clone(),
                    to: to.clone(),
                });
            }
            _ => {}
        }
    }

    // 6. Build new desired state lockfile
    let mut updated_artifacts = current_lockfile
        .as_ref()
        .map(|l| l.artifacts.clone())
        .unwrap_or_default();

    // Retain only non-content artifacts or unaffected content artifacts
    updated_artifacts.retain(|a| !a.destination.as_str().starts_with(".minecraft/mods/"));

    for pentry in &plan.planned_entries {
        let content_file = plan
            .artifacts_to_acquire
            .iter()
            .find(|a| pentry.destination.as_str().ends_with(&a.filename));

        let (sources, integrity, expected_size) = if let Some(cf) = content_file {
            (cf.sources.clone(), cf.integrity.clone(), Some(cf.size))
        } else {
            (Vec::new(), graphene_core::ArtifactIntegrity::none(), None)
        };

        updated_artifacts.push(LockedArtifact {
            logical_key: pentry.artifact_logical_key.clone(),
            kind: ArtifactKind::Binary,
            sources,
            destination: pentry.destination.clone(),
            scope: LockedMaterializationScope::InstanceMutable,
            integrity,
            expected_size,
        });
    }

    let new_lockfile = InstanceLockfile {
        schema_version: LOCKFILE_SCHEMA_VERSION,
        instance_id: plan.instance_id,
        minecraft_version: receipt.requested_version.clone(),
        components: receipt.components.clone(),
        artifacts: updated_artifacts,
        native_extractions: current_lockfile
            .as_ref()
            .map(|l| l.native_extractions.clone())
            .unwrap_or_default(),
        generated_outputs: current_lockfile
            .as_ref()
            .map(|l| l.generated_outputs.clone())
            .unwrap_or_default(),
        content: plan.resulting_lockfile_entries.clone(),
    };

    new_lockfile.validate()?;
    let new_lockfile_bytes = serde_json::to_vec_pretty(&new_lockfile).map_err(|source| {
        GrapheneError::new(
            ErrorCode::InstanceMutationFailed,
            ErrorKind::Instance,
            "failed to serialize new instance lockfile",
        )
        .with_source(source)
    })?;

    let old_lockfile_sha256 = lockfile_bytes.as_deref().map(compute_sha256);
    let new_lockfile_sha256 = compute_sha256(&new_lockfile_bytes);

    // 7. Write durable journal (Prepared)
    operation.set_stage("write-transaction-journal")?;
    let journal = ContentMutationJournal {
        schema_version: crate::content_service::transaction::JOURNAL_SCHEMA_VERSION,
        operation_id: operation.handle().id().to_string(),
        instance_id: plan.instance_id,
        phase: ContentTransactionPhase::Prepared,
        old_lockfile_sha256,
        new_lockfile_sha256,
        staged_mappings: staged_mappings.clone(),
        quarantine_mappings: quarantine_mappings.clone(),
        rename_mappings: rename_mappings.clone(),
    };

    if fault_point == Some(ContentFaultPoint::BeforeJournalDurable) {
        return Err(GrapheneError::new(
            ErrorCode::InternalInvariantViolation,
            ErrorKind::Internal,
            "injected fault BeforeJournalDurable",
        ));
    }

    journal.write_durable(&instance_root)?;

    if fault_point == Some(ContentFaultPoint::AfterJournalPrepared) {
        return Err(GrapheneError::new(
            ErrorCode::InternalInvariantViolation,
            ErrorKind::Internal,
            "injected fault AfterJournalPrepared",
        ));
    }

    // 8. Move quarantined files
    operation.set_stage("quarantine-files")?;
    let quarantine_dir = instance_root
        .join(".graphene")
        .join("quarantine")
        .join("content");
    fs::create_dir_all(&quarantine_dir).ok();

    for qm in &quarantine_mappings {
        let orig = instance_root.join(qm.original_path.as_str());
        let q_target = instance_root.join(&qm.quarantine_path);
        if orig.exists() {
            if let Some(parent) = q_target.parent() {
                let _ = fs::create_dir_all(parent);
            }
            replace_file_safely(&orig, &q_target)?;
        }
    }

    if fault_point == Some(ContentFaultPoint::AfterQuarantine) {
        return Err(GrapheneError::new(
            ErrorCode::InternalInvariantViolation,
            ErrorKind::Internal,
            "injected fault AfterQuarantine",
        ));
    }

    // 9. Publish staged files to final destination
    operation.set_stage("publish-content")?;
    for sm in &staged_mappings {
        let staged = instance_root.join(&sm.staged_path);
        let dest = instance_root.join(sm.destination.as_str());
        if staged.exists() {
            if let Some(parent) = dest.parent() {
                let _ = fs::create_dir_all(parent);
            }
            replace_file_safely(&staged, &dest)?;
        }
    }

    // 10. Perform enable/disable renames
    for rm in &rename_mappings {
        let from_path = instance_root.join(rm.from.as_str());
        let to_path = instance_root.join(rm.to.as_str());
        if from_path.exists() {
            if let Some(parent) = to_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            replace_file_safely(&from_path, &to_path)?;
        }
    }

    if fault_point == Some(ContentFaultPoint::AfterPartialPublication) {
        return Err(GrapheneError::new(
            ErrorCode::InternalInvariantViolation,
            ErrorKind::Internal,
            "injected fault AfterPartialPublication",
        ));
    }

    if fault_point == Some(ContentFaultPoint::BeforeLockfileCommit) {
        return Err(GrapheneError::new(
            ErrorCode::InternalInvariantViolation,
            ErrorKind::Internal,
            "injected fault BeforeLockfileCommit",
        ));
    }

    // 11. Atomically commit new lockfile (point-of-no-return commit boundary)
    operation.set_stage("commit-lockfile")?;
    write_json_atomic(&lockfile_path, &new_lockfile)?;

    if fault_point == Some(ContentFaultPoint::AfterLockfileCommitBeforeFinalize) {
        return Err(GrapheneError::new(
            ErrorCode::InternalInvariantViolation,
            ErrorKind::Internal,
            "injected fault AfterLockfileCommitBeforeFinalize",
        ));
    }

    // 12. Finalize journal & cleanup
    operation.set_stage("cleanup-transaction")?;
    journal.delete_durable(&instance_root).ok();
    let _ = fs::remove_dir_all(&staging_dir);
    let _ = fs::remove_dir_all(&quarantine_dir);

    let final_effective_config = repo.effective_config(plan.instance_id).ok();
    let committed_fingerprint = InstanceStateFingerprint::compute(
        plan.instance_id,
        &receipt,
        Some(&new_lockfile),
        final_effective_config.as_ref(),
    );

    let modified_entry_ids = plan
        .planned_entries
        .iter()
        .map(|e| e.entry_id.clone())
        .collect();

    Ok(ContentMutationResult {
        modified_entry_ids,
        committed_fingerprint,
    })
}

fn compute_sha256(bytes: &[u8]) -> Sha256Digest {
    let hash: [u8; 32] = Sha256::digest(bytes).into();
    Sha256Digest::from_bytes(hash)
}
