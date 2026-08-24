use crate::id::{ContentProjectRef, ContentVersionRef};
use serde::{Deserialize, Serialize};

/// Classification of a dependency relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DependencyRelation {
    /// Mandatory dependency required for the content to function.
    #[default]
    Required,
    /// Optional or recommended dependency not required for execution.
    Optional,
    /// Known conflicting or incompatible content.
    Incompatible,
    /// Bundled or embedded inside the primary artifact.
    Embedded,
    /// Unknown or unmapped provider relationship.
    Unknown,
}

/// Bounded normalized dependency target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyTarget {
    /// Points to an exact remote version.
    Version(ContentVersionRef),
    /// Points to a project where version must be resolved.
    Project(ContentProjectRef),
    /// External or filename hint when project identity is unmapped.
    FilenameHint(String),
}

/// Normalized content dependency descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentDependency {
    pub target: DependencyTarget,
    pub relation: DependencyRelation,
}

impl ContentDependency {
    #[must_use]
    pub const fn new(target: DependencyTarget, relation: DependencyRelation) -> Self {
        Self { target, relation }
    }

    #[must_use]
    pub fn required_project(project: ContentProjectRef) -> Self {
        Self {
            target: DependencyTarget::Project(project),
            relation: DependencyRelation::Required,
        }
    }

    #[must_use]
    pub fn required_version(version: ContentVersionRef) -> Self {
        Self {
            target: DependencyTarget::Version(version),
            relation: DependencyRelation::Required,
        }
    }
}
