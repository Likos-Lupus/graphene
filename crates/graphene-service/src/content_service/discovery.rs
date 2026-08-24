use crate::context::ServiceContext;
use graphene_content::{
    ContentProject, ContentProjectRef, ContentProviderId, ContentSearchPage, ContentSearchQuery,
    ContentVersion, ContentVersionFilter, ContentVersionRef,
};
use graphene_core::{OperationController, Result};
use std::sync::Arc;

pub async fn search_content(
    context: &Arc<ServiceContext>,
    provider_id: &ContentProviderId,
    query: &ContentSearchQuery,
    operation: &OperationController,
) -> Result<ContentSearchPage> {
    let provider = context.content_registry.get(provider_id)?;
    provider.search(query, operation).await
}

pub async fn get_content_project(
    context: &Arc<ServiceContext>,
    project_ref: &ContentProjectRef,
    operation: &OperationController,
) -> Result<ContentProject> {
    let provider = context.content_registry.get(&project_ref.provider)?;
    provider.project(project_ref, operation).await
}

pub async fn list_content_versions(
    context: &Arc<ServiceContext>,
    project_ref: &ContentProjectRef,
    filters: &ContentVersionFilter,
    operation: &OperationController,
) -> Result<Vec<ContentVersion>> {
    let provider = context.content_registry.get(&project_ref.provider)?;
    provider
        .list_versions(project_ref, filters, operation)
        .await
}

pub async fn get_content_version(
    context: &Arc<ServiceContext>,
    version_ref: &ContentVersionRef,
    operation: &OperationController,
) -> Result<ContentVersion> {
    let provider = context.content_registry.get(&version_ref.provider)?;
    provider.get_version(version_ref, operation).await
}
