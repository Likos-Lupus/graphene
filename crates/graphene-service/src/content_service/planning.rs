use crate::{
    content_service::inventory::scan_instance_inventory, context::ServiceContext,
    instance_service::repository::InstanceRepository,
};
use graphene_content::{
    CONTENT_PLAN_SCHEMA_VERSION, ContentActionRequest, ContentEntryId, ContentKind,
    ContentMutationPlan, ContentMutationRequest, InstanceContentContext, PlannedContentEntry,
    PlannedFilesystemAction, dependency::resolve_dependencies, plan::model::sanitize_mod_filename,
};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, OperationController, Result};
use graphene_instance::{
    InstalledComponentKind, InstanceLockfile, InstanceStateFingerprint, LockedContentEntry,
    ManagedRelativePath,
};
use graphene_storage::{MAX_LOCKFILE_BYTES, read_document_bounded};
use std::collections::HashMap;
use std::sync::Arc;

/// Derives a deterministic, non-mutating `ContentMutationPlan` from current instance state and request.
pub async fn plan_content_mutation(
    context: &Arc<ServiceContext>,
    request: &ContentMutationRequest,
    operation: &OperationController,
) -> Result<ContentMutationPlan> {
    let repo = InstanceRepository::new(context.storage.path());
    let _lease = repo.acquire_shared_lease(request.instance_id)?;

    let receipt = repo.load_receipt(request.instance_id)?;
    let lockfile_path = repo.paths().lockfile_path(request.instance_id);
    let lockfile = if lockfile_path.exists() {
        let bytes = read_document_bounded(&lockfile_path, MAX_LOCKFILE_BYTES)?;
        serde_json::from_slice::<InstanceLockfile>(&bytes).ok()
    } else {
        None
    };

    let effective_config = repo.effective_config(request.instance_id).ok();
    let base_state_fingerprint = InstanceStateFingerprint::compute(
        request.instance_id,
        &receipt,
        lockfile.as_ref(),
        effective_config.as_ref(),
    );

    // 1. Scan offline local inventory
    let inventory = scan_instance_inventory(context, request.instance_id, true, operation).await?;
    let base_inventory_fingerprint = inventory.fingerprint;

    // 2. Build content context from committed components
    let primary_loader_component = receipt
        .components
        .iter()
        .find(|c| c.kind == InstalledComponentKind::Loader);

    let loader_kind = primary_loader_component.and_then(|c| match c.uid.as_str() {
        "net.fabricmc.fabric-loader" => Some(graphene_minecraft::LoaderKind::Fabric),
        "net.minecraftforge.forge" => Some(graphene_minecraft::LoaderKind::Forge),
        "net.neoforged.neoforge" => Some(graphene_minecraft::LoaderKind::NeoForge),
        _ => None,
    });

    let exact_loader_version = primary_loader_component.map(|c| c.version.clone());
    let content_context = InstanceContentContext::new(
        request.instance_id,
        receipt.requested_version.clone(),
        loader_kind,
        exact_loader_version,
    );

    // 3. Initialize working state from existing lockfile content
    let mut working_entries: HashMap<ContentEntryId, PlannedContentEntry> = HashMap::new();
    if let Some(lock) = &lockfile {
        for entry in &lock.content {
            if let Ok(eid) = ContentEntryId::new(&entry.entry_id) {
                let provider_id = entry
                    .provider
                    .as_ref()
                    .and_then(|p| graphene_content::ContentProviderId::new(p).ok());
                working_entries.insert(
                    eid.clone(),
                    PlannedContentEntry {
                        entry_id: eid,
                        kind: ContentKind::Mod,
                        provider: provider_id,
                        project_id: entry.project_id.clone(),
                        version_id: entry.version_id.clone(),
                        file_id: entry.file_id.clone(),
                        artifact_logical_key: entry.artifact_logical_key.clone(),
                        destination: entry.destination.clone(),
                        enabled: entry.enabled,
                        dependencies: Vec::new(),
                    },
                );
            }
        }
    }

    let mut filesystem_actions: Vec<PlannedFilesystemAction> = Vec::new();
    let mut artifacts_to_acquire = Vec::new();
    let mut estimated_download_bytes = 0u64;
    let mut diagnostics = Vec::new();

    // 4. Process each action request
    for action in &request.actions {
        match action {
            ContentActionRequest::InstallExactVersion(vref) => {
                let provider = context.content_registry.get(&vref.provider)?;
                let root_ver = provider.get_version(vref, operation).await?;

                let closure = resolve_dependencies(
                    &[root_ver],
                    &content_context,
                    request.policy,
                    provider.as_ref(),
                    operation,
                )
                .await?;

                for ver in closure.versions {
                    let file = ver.primary_file().ok_or_else(|| {
                        GrapheneError::new(
                            ErrorCode::ContentFileUnverifiable,
                            ErrorKind::Content,
                            format!(
                                "version {} does not contain a primary verifiable file",
                                ver.version_ref
                            ),
                        )
                    })?;

                    let safe_filename = sanitize_mod_filename(
                        &file.filename,
                        &ver.version_ref.project_id,
                        file.integrity.sha256().as_ref(),
                        true,
                    );
                    let dest_rel = format!(".minecraft/mods/{safe_filename}");
                    let destination = ManagedRelativePath::new(&dest_rel)?;

                    let entry_id = ContentEntryId::generate();
                    let logical_key = format!(
                        "content:mod:{}:{}",
                        ver.version_ref.project_id, ver.version_ref.version_id
                    );

                    // Check if file already exists in local inventory with exact matching SHA-1
                    let local_match = inventory
                        .files
                        .iter()
                        .find(|lf| lf.sha1.is_some() && lf.sha1 == file.integrity.sha1());

                    if let Some(local_file) = local_match {
                        filesystem_actions.push(PlannedFilesystemAction::AdoptExistingFile {
                            path: local_file.relative_path.clone(),
                        });
                    } else {
                        artifacts_to_acquire.push(file.clone());
                        estimated_download_bytes += file.size;

                        filesystem_actions.push(PlannedFilesystemAction::StageArtifact {
                            artifact_logical_key: logical_key.clone(),
                            destination: destination.clone(),
                        });
                        filesystem_actions.push(PlannedFilesystemAction::PublishStagedArtifact {
                            staged_relative: format!(".graphene/staging/content/{safe_filename}"),
                            final_destination: destination.clone(),
                        });
                    }

                    working_entries.insert(
                        entry_id.clone(),
                        PlannedContentEntry {
                            entry_id,
                            kind: ContentKind::Mod,
                            provider: Some(ver.version_ref.provider.clone()),
                            project_id: Some(ver.version_ref.project_id.clone()),
                            version_id: Some(ver.version_ref.version_id.clone()),
                            file_id: Some(file.file_ref.file_id.clone()),
                            artifact_logical_key: logical_key,
                            destination,
                            enabled: true,
                            dependencies: ver.dependencies.clone(),
                        },
                    );
                }

                for opt_dep in closure.optional_dependencies {
                    diagnostics.push(format!(
                        "optional dependency surfaced: {:?}",
                        opt_dep.target
                    ));
                }
            }
            ContentActionRequest::AdoptRecognizedLocal {
                path,
                expected_sha256,
                file_ref,
            } => {
                let local_file = inventory.find_by_path(path).ok_or_else(|| {
                    GrapheneError::new(
                        ErrorCode::InstanceNotFound,
                        ErrorKind::Content,
                        format!("local file {} not found in inventory", path.as_str()),
                    )
                })?;

                if local_file.sha256.as_ref() != Some(expected_sha256) {
                    return Err(GrapheneError::new(
                        ErrorCode::ContentInventoryStale,
                        ErrorKind::Content,
                        format!(
                            "local file {} SHA-256 differs from expected adoption digest",
                            path.as_str()
                        ),
                    ));
                }

                let provider = context.content_registry.get(&file_ref.provider)?;
                let version = provider
                    .get_version(&file_ref.version_ref(), operation)
                    .await?;

                let entry_id = ContentEntryId::generate();
                let logical_key = format!(
                    "content:mod:{}:{}",
                    file_ref.project_id, file_ref.version_id
                );

                filesystem_actions
                    .push(PlannedFilesystemAction::AdoptExistingFile { path: path.clone() });

                working_entries.insert(
                    entry_id.clone(),
                    PlannedContentEntry {
                        entry_id,
                        kind: ContentKind::Mod,
                        provider: Some(file_ref.provider.clone()),
                        project_id: Some(file_ref.project_id.clone()),
                        version_id: Some(file_ref.version_id.clone()),
                        file_id: Some(file_ref.file_id.clone()),
                        artifact_logical_key: logical_key,
                        destination: path.clone(),
                        enabled: local_file.enabled,
                        dependencies: version.dependencies,
                    },
                );
            }
            ContentActionRequest::UpdateManaged {
                entry_id,
                target_version,
            } => {
                let existing = working_entries
                    .get(entry_id)
                    .ok_or_else(|| {
                        GrapheneError::new(
                            ErrorCode::ContentProjectNotFound,
                            ErrorKind::Content,
                            format!("managed entry {entry_id} not found in instance desired state"),
                        )
                    })?
                    .clone();

                let provider_id = existing.provider.as_ref().ok_or_else(|| {
                    GrapheneError::new(
                        ErrorCode::ContentProviderUnavailable,
                        ErrorKind::Content,
                        format!("managed entry {entry_id} has no remote provider identity"),
                    )
                })?;

                let provider = context.content_registry.get(provider_id)?;

                let target_ver = if let Some(tv) = target_version {
                    provider.get_version(tv, operation).await?
                } else {
                    let cur_file_ref = graphene_content::ContentFileRef::new(
                        provider_id.clone(),
                        existing.project_id.as_deref().unwrap_or(""),
                        existing.version_id.as_deref().unwrap_or(""),
                        existing.file_id.as_deref().unwrap_or(""),
                    )?;
                    let opt_up = provider
                        .find_update(&cur_file_ref, &content_context, request.policy, operation)
                        .await?;
                    match opt_up {
                        Some(up) => up,
                        None => {
                            diagnostics.push(format!("no update available for entry {entry_id}"));
                            continue; // No-op for this entry
                        }
                    }
                };

                // If target version is identical to existing version, no-op
                if existing.version_id.as_deref() == Some(&target_ver.version_ref.version_id) {
                    continue;
                }

                let closure = resolve_dependencies(
                    &[target_ver],
                    &content_context,
                    request.policy,
                    provider.as_ref(),
                    operation,
                )
                .await?;

                // Quarantine old file
                filesystem_actions.push(PlannedFilesystemAction::QuarantineFile {
                    path: existing.destination.clone(),
                });

                for ver in closure.versions {
                    let file = ver.primary_file().ok_or_else(|| {
                        GrapheneError::new(
                            ErrorCode::ContentFileUnverifiable,
                            ErrorKind::Content,
                            format!(
                                "version {} does not contain a primary verifiable file",
                                ver.version_ref
                            ),
                        )
                    })?;

                    let safe_filename = sanitize_mod_filename(
                        &file.filename,
                        &ver.version_ref.project_id,
                        file.integrity.sha256().as_ref(),
                        existing.enabled,
                    );
                    let dest_rel = format!(".minecraft/mods/{safe_filename}");
                    let destination = ManagedRelativePath::new(&dest_rel)?;

                    let logical_key = format!(
                        "content:mod:{}:{}",
                        ver.version_ref.project_id, ver.version_ref.version_id
                    );

                    artifacts_to_acquire.push(file.clone());
                    estimated_download_bytes += file.size;

                    filesystem_actions.push(PlannedFilesystemAction::StageArtifact {
                        artifact_logical_key: logical_key.clone(),
                        destination: destination.clone(),
                    });
                    filesystem_actions.push(PlannedFilesystemAction::PublishStagedArtifact {
                        staged_relative: format!(".graphene/staging/content/{safe_filename}"),
                        final_destination: destination.clone(),
                    });

                    working_entries.insert(
                        entry_id.clone(),
                        PlannedContentEntry {
                            entry_id: entry_id.clone(),
                            kind: ContentKind::Mod,
                            provider: Some(ver.version_ref.provider.clone()),
                            project_id: Some(ver.version_ref.project_id.clone()),
                            version_id: Some(ver.version_ref.version_id.clone()),
                            file_id: Some(file.file_ref.file_id.clone()),
                            artifact_logical_key: logical_key,
                            destination,
                            enabled: existing.enabled,
                            dependencies: ver.dependencies,
                        },
                    );
                }
            }
            ContentActionRequest::SetEnabled { entry_id, enabled } => {
                let existing = working_entries.get_mut(entry_id).ok_or_else(|| {
                    GrapheneError::new(
                        ErrorCode::ContentProjectNotFound,
                        ErrorKind::Content,
                        format!("managed entry {entry_id} not found in instance desired state"),
                    )
                })?;

                if existing.enabled != *enabled {
                    let old_path = existing.destination.clone();
                    let file_name = old_path.as_str().trim_start_matches(".minecraft/mods/");
                    let new_file_name = if *enabled {
                        file_name.trim_end_matches(".disabled").to_string()
                    } else if !file_name.ends_with(".disabled") {
                        format!("{file_name}.disabled")
                    } else {
                        file_name.to_string()
                    };

                    let new_destination =
                        ManagedRelativePath::new(format!(".minecraft/mods/{new_file_name}"))?;

                    filesystem_actions.push(PlannedFilesystemAction::RenameFile {
                        from: old_path,
                        to: new_destination.clone(),
                    });

                    existing.enabled = *enabled;
                    existing.destination = new_destination;
                }
            }
            ContentActionRequest::RemoveManaged { entry_id, cascade } => {
                let existing = working_entries
                    .get(entry_id)
                    .ok_or_else(|| {
                        GrapheneError::new(
                            ErrorCode::ContentProjectNotFound,
                            ErrorKind::Content,
                            format!("managed entry {entry_id} not found in instance desired state"),
                        )
                    })?
                    .clone();

                // Reverse dependency protection: check if other enabled entries require this mod
                if !cascade && let Some(pid) = &existing.project_id {
                    for (other_id, other_entry) in &working_entries {
                        if other_id == entry_id || !other_entry.enabled {
                            continue;
                        }
                        for dep in &other_entry.dependencies {
                            if dep.relation == graphene_content::DependencyRelation::Required {
                                match &dep.target {
                                    graphene_content::DependencyTarget::Project(p)
                                        if &p.project_id == pid =>
                                    {
                                        return Err(GrapheneError::new(
                                            ErrorCode::ContentDependencyConflict,
                                            ErrorKind::Content,
                                            format!(
                                                "cannot remove managed entry {entry_id}: required by active entry {other_id}"
                                            ),
                                        ));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }

                filesystem_actions.push(PlannedFilesystemAction::QuarantineFile {
                    path: existing.destination.clone(),
                });
                filesystem_actions.push(PlannedFilesystemAction::DeleteQuarantinedFile {
                    path: existing.destination.clone(),
                });

                working_entries.remove(entry_id);
            }
            ContentActionRequest::RemoveLocalExact {
                path,
                expected_sha256,
                expected_size,
            } => {
                let local_file = inventory.find_by_path(path).ok_or_else(|| {
                    GrapheneError::new(
                        ErrorCode::InstanceNotFound,
                        ErrorKind::Content,
                        format!("unmanaged file {} not found in inventory", path.as_str()),
                    )
                })?;

                if local_file.sha256.as_ref() != Some(expected_sha256)
                    || local_file.size != *expected_size
                {
                    return Err(GrapheneError::new(
                        ErrorCode::ContentInventoryStale,
                        ErrorKind::Content,
                        format!(
                            "unmanaged file {} content has changed from snapshot",
                            path.as_str()
                        ),
                    ));
                }

                filesystem_actions
                    .push(PlannedFilesystemAction::QuarantineFile { path: path.clone() });
                filesystem_actions
                    .push(PlannedFilesystemAction::DeleteQuarantinedFile { path: path.clone() });
            }
        }
    }

    let mut planned_entries: Vec<PlannedContentEntry> = working_entries.into_values().collect();
    planned_entries.sort_by(|a, b| a.entry_id.as_str().cmp(b.entry_id.as_str()));

    let resulting_lockfile_entries: Vec<LockedContentEntry> = planned_entries
        .iter()
        .map(PlannedContentEntry::to_locked_entry)
        .collect();

    let plan = ContentMutationPlan {
        schema_version: CONTENT_PLAN_SCHEMA_VERSION,
        instance_id: request.instance_id,
        base_state_fingerprint,
        base_inventory_fingerprint,
        context: content_context,
        requested_actions: request.actions.clone(),
        planned_entries,
        filesystem_actions,
        artifacts_to_acquire,
        resulting_lockfile_entries,
        diagnostics,
        estimated_download_bytes,
    };

    plan.validate()?;
    Ok(plan)
}
