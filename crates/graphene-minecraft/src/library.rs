use crate::{MavenCoordinate, MinecraftOs, ResolvedArtifact, Rule};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Provider-normalized library download metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Library {
    pub coordinate: MavenCoordinate,
    pub rules: Vec<Rule>,
    pub artifact: Option<ResolvedArtifact>,
    pub classifiers: BTreeMap<String, ResolvedArtifact>,
    pub natives: BTreeMap<MinecraftOs, String>,
}

/// Library contribution after rules/native selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedLibrary {
    pub coordinate: MavenCoordinate,
    pub classpath_artifact: Option<ResolvedArtifact>,
    pub native_artifact: Option<ResolvedArtifact>,
}
