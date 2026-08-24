use crate::{
    id::{ContentProjectRef, ContentVersionRef},
    model::{dependency::ContentDependency, file::ContentFile, version::ContentVersion},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Result of deterministic dependency resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedDependencyClosure {
    /// Pinned exact versions in deterministic topological / sorted order.
    pub versions: Vec<ContentVersion>,
    /// Exact primary files to be acquired and installed.
    pub files: Vec<ContentFile>,
    /// Optional dependencies discovered during traversal (not auto-installed).
    pub optional_dependencies: Vec<ContentDependency>,
}

/// In-flight state tracking during dependency resolution.
#[derive(Debug, Default)]
pub struct DependencyResolutionGraph {
    pub visited_projects: BTreeSet<ContentProjectRef>,
    pub pinned_versions: BTreeMap<ContentProjectRef, ContentVersionRef>,
    pub versions_by_ref: BTreeMap<ContentVersionRef, ContentVersion>,
    pub optional_dependencies: Vec<ContentDependency>,
    pub request_count: usize,
}
