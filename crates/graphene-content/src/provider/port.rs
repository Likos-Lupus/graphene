use crate::{
    id::{ContentFileRef, ContentProjectRef, ContentProviderId, ContentVersionRef},
    model::{
        compatibility::{InstanceContentContext, ReleaseChannelPolicy},
        project::{ContentProject, ContentSearchPage},
        version::ContentVersion,
    },
    provider::{
        capabilities::ContentProviderCapabilities,
        match_request::{ContentFileMatch, FileMatchRequest},
    },
};
use graphene_core::{OperationController, Result};
use graphene_minecraft::LoaderKind;
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin};

pub type ContentProviderFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/// Search parameters for querying content projects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ContentSearchQuery {
    pub query: String,
    pub minecraft_version: Option<String>,
    pub loader: Option<LoaderKind>,
    pub offset: u32,
    pub limit: u32,
}

/// Filter criteria for listing project versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ContentVersionFilter {
    pub minecraft_version: Option<String>,
    pub loader: Option<LoaderKind>,
}

/// Asynchronous provider port for remote content catalogs.
pub trait ContentProvider: Send + Sync + std::fmt::Debug {
    /// Returns the stable provider identifier.
    fn id(&self) -> &ContentProviderId;

    /// Returns the provider's feature capabilities.
    fn capabilities(&self) -> ContentProviderCapabilities;

    /// Searches the provider catalog.
    fn search<'a>(
        &'a self,
        query: &'a ContentSearchQuery,
        operation: &'a OperationController,
    ) -> ContentProviderFuture<'a, ContentSearchPage>;

    /// Fetches project details by project reference.
    fn project<'a>(
        &'a self,
        project_ref: &'a ContentProjectRef,
        operation: &'a OperationController,
    ) -> ContentProviderFuture<'a, ContentProject>;

    /// Lists versions of a project matching the filter criteria.
    fn list_versions<'a>(
        &'a self,
        project_ref: &'a ContentProjectRef,
        filters: &'a ContentVersionFilter,
        operation: &'a OperationController,
    ) -> ContentProviderFuture<'a, Vec<ContentVersion>>;

    /// Fetches exact version details.
    fn get_version<'a>(
        &'a self,
        version_ref: &'a ContentVersionRef,
        operation: &'a OperationController,
    ) -> ContentProviderFuture<'a, ContentVersion>;

    /// Matches local file digests/fingerprints against the catalog.
    fn match_files<'a>(
        &'a self,
        request: &'a FileMatchRequest,
        operation: &'a OperationController,
    ) -> ContentProviderFuture<'a, Vec<ContentFileMatch>>;

    /// Looks up the best compatible update for an existing file.
    fn find_update<'a>(
        &'a self,
        current_file: &'a ContentFileRef,
        context: &'a InstanceContentContext,
        policy: ReleaseChannelPolicy,
        operation: &'a OperationController,
    ) -> ContentProviderFuture<'a, Option<ContentVersion>>;
}
