use crate::{
    id::ContentProjectRef,
    model::{kind::ContentKind, version::EnvironmentSupport},
};
use graphene_minecraft::LoaderKind;
use serde::{Deserialize, Serialize};

/// Detailed metadata describing a content project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentProject {
    pub project_ref: ContentProjectRef,
    pub kind: ContentKind,
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub icon_url: Option<String>,
    pub website_url: Option<String>,
    pub source_url: Option<String>,
    pub issues_url: Option<String>,
    pub wiki_url: Option<String>,
    pub authors: Vec<String>,
    pub categories: Vec<String>,
    pub downloads: Option<u64>,
    pub follows: Option<u64>,
    pub client_side: EnvironmentSupport,
    pub server_side: EnvironmentSupport,
    pub game_versions: Vec<String>,
    pub loaders: Vec<LoaderKind>,
}

/// A single item in a content search result page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentSearchHit {
    pub project_ref: ContentProjectRef,
    pub kind: ContentKind,
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub icon_url: Option<String>,
    pub authors: Vec<String>,
    pub categories: Vec<String>,
    pub downloads: Option<u64>,
    pub follows: Option<u64>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<LoaderKind>,
}

/// Paginated search result page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentSearchPage {
    pub hits: Vec<ContentSearchHit>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: Option<u64>,
}
