use crate::{
    ManagedPath, MavenCoordinate, MinecraftJavaRequirement, ResolvedArtifact, ResolvedComponent,
};
use graphene_core::ArtifactIntegrity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PreparationDataValue {
    Literal(String),
    MavenCoordinate(MavenCoordinate),
    EmbeddedInstallerEntry(String),
    ManagedPath(ManagedPath),
    SideSpecific {
        client: Box<PreparationDataValue>,
        server: Option<Box<PreparationDataValue>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PreparationPlaceholder {
    Root,
    Installer,
    LibraryDir,
    MinecraftJar,
    MinecraftVersion,
    Side,
    MavenPath(MavenCoordinate),
    Data(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PreparationArgumentPart {
    Literal(String),
    Placeholder(PreparationPlaceholder),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PreparationArgument {
    Literal(String),
    Placeholder(PreparationPlaceholder),
    Template(Vec<PreparationArgumentPart>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ProcessorSideCondition {
    Any,
    Client,
    Server,
}

impl ProcessorSideCondition {
    #[must_use]
    pub const fn executes_for_client(&self) -> bool {
        !matches!(self, Self::Server)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddedInstallerInput {
    pub entry: String,
    pub staging_path: ManagedPath,
    pub expected_integrity: ArtifactIntegrity,
    pub maximum_size: u64,
    pub consumers: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GeneratedOutputScope {
    StagingOnly,
    SharedImmutable,
    InstanceStaging,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedOutput {
    pub id: String,
    pub producer: String,
    pub input_identity: String,
    pub staging_path: ManagedPath,
    pub managed_destination: ManagedPath,
    pub scope: GeneratedOutputScope,
    pub expected_size: Option<u64>,
    pub expected_integrity: ArtifactIntegrity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessorStep {
    pub id: String,
    pub java_requirement: Option<MinecraftJavaRequirement>,
    pub executable_jar: ResolvedArtifact,
    pub classpath: Vec<ResolvedArtifact>,
    pub main_class: Option<String>,
    pub arguments: Vec<PreparationArgument>,
    pub side: ProcessorSideCondition,
    pub declared_outputs: Vec<String>,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ComponentPreparationRecipe {
    pub component: Option<ResolvedComponent>,
    pub installer: Option<ResolvedArtifact>,
    pub embedded_inputs: Vec<EmbeddedInstallerInput>,
    pub input_artifacts: Vec<ResolvedArtifact>,
    pub data: BTreeMap<String, PreparationDataValue>,
    pub processors: Vec<ProcessorStep>,
    pub generated_outputs: Vec<GeneratedOutput>,
    pub installer_java_requirement: Option<MinecraftJavaRequirement>,
}
