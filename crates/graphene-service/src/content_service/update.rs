use crate::{
    content_service::planning::plan_content_mutation, context::ServiceContext,
    instance_service::repository::InstanceRepository,
};
use graphene_content::{
    ContentActionRequest, ContentEntryId, ContentMutationPlan, ContentMutationRequest,
    ContentVersion, ReleaseChannelPolicy,
};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, InstanceId, OperationController, Result};
use graphene_instance::{InstalledComponentKind, InstanceLockfile};
use graphene_storage::{MAX_LOCKFILE_BYTES, read_document_bounded};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// An available update discovered for a managed content entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailableContentUpdate {
    pub entry_id: ContentEntryId,
    pub current_version_id: Option<String>,
    pub target_version: ContentVersion,
}

/// Finds available compatible updates for all Graphene-managed mods in an instance.
pub async fn find_available_updates(
    context: &Arc<ServiceContext>,
    instance_id: InstanceId,
    policy: ReleaseChannelPolicy,
    operation: &OperationController,
) -> Result<Vec<AvailableContentUpdate>> {
    let repo = InstanceRepository::new(context.storage.path());
    let _lease = repo.acquire_shared_lease(instance_id)?;

    let receipt = repo.load_receipt(instance_id)?;
    let lockfile_path = repo.paths().lockfile_path(instance_id);
    if !lockfile_path.exists() {
        return Ok(Vec::new());
    }

    let bytes = read_document_bounded(&lockfile_path, MAX_LOCKFILE_BYTES)?;
    let lockfile: InstanceLockfile = serde_json::from_slice(&bytes).map_err(|source| {
        GrapheneError::new(
            ErrorCode::InstanceLockfileInvalid,
            ErrorKind::Instance,
            "failed to deserialize instance lockfile",
        )
        .with_source(source)
    })?;

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
    let content_context = graphene_content::InstanceContentContext::new(
        instance_id,
        receipt.requested_version.clone(),
        loader_kind,
        exact_loader_version,
    );

    let mut updates = Vec::new();

    for entry in &lockfile.content {
        let entry_id = match ContentEntryId::new(&entry.entry_id) {
            Ok(id) => id,
            Err(_) => continue,
        };

        let provider_id_str = match &entry.provider {
            Some(p) => p,
            None => continue,
        };

        let provider_id = match graphene_content::ContentProviderId::new(provider_id_str) {
            Ok(id) => id,
            Err(_) => continue,
        };

        let provider = match context.content_registry.get(&provider_id) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let project_id = match &entry.project_id {
            Some(pid) => pid,
            None => continue,
        };

        let cur_file_ref = graphene_content::ContentFileRef::new(
            provider_id.clone(),
            project_id,
            entry.version_id.as_deref().unwrap_or(""),
            entry.file_id.as_deref().unwrap_or(""),
        )?;

        if let Ok(Some(update_ver)) = provider
            .find_update(&cur_file_ref, &content_context, policy, operation)
            .await
            && entry.version_id.as_deref() != Some(&update_ver.version_ref.version_id)
        {
            updates.push(AvailableContentUpdate {
                entry_id,
                current_version_id: entry.version_id.clone(),
                target_version: update_ver,
            });
        }
    }

    Ok(updates)
}

/// Builds a batch update plan for all available mod updates.
pub async fn plan_updates(
    context: &Arc<ServiceContext>,
    instance_id: InstanceId,
    policy: ReleaseChannelPolicy,
    operation: &OperationController,
) -> Result<ContentMutationPlan> {
    let updates = find_available_updates(context, instance_id, policy, operation).await?;

    let actions = updates
        .into_iter()
        .map(|u| ContentActionRequest::UpdateManaged {
            entry_id: u.entry_id,
            target_version: Some(u.target_version.version_ref),
        })
        .collect();

    let request = ContentMutationRequest {
        instance_id,
        actions,
        policy,
    };

    plan_content_mutation(context, &request, operation).await
}
