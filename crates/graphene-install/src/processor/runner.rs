use graphene_core::{CancellationToken, Result};
use graphene_minecraft::MinecraftJavaRequirement;
use std::{future::Future, path::PathBuf, pin::Pin, time::Duration};

/// Graphene-install-owned view of a Java runtime selected through the host's Java boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedToolJava {
    pub executable: PathBuf,
    pub major_version: u32,
}

#[derive(Debug, Clone)]
pub struct ToolRunRequest {
    pub processor_id: String,
    pub java: SelectedToolJava,
    pub executable_jar: PathBuf,
    pub classpath: Vec<PathBuf>,
    pub main_class: String,
    pub arguments: Vec<String>,
    pub working_directory: PathBuf,
    pub timeout: Duration,
    pub cancellation: CancellationToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRunResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

pub type ToolJavaFuture<'a> = Pin<Box<dyn Future<Output = Result<SelectedToolJava>> + Send + 'a>>;
pub type ToolRunnerFuture<'a> = Pin<Box<dyn Future<Output = Result<ToolRunResult>> + Send + 'a>>;

/// Dependency-inversion port for selecting Java through Phase 2 and launching one verified Java tool.
pub trait InstallToolRunner: Send + Sync {
    fn select_java<'a>(
        &'a self,
        requirement: Option<&'a MinecraftJavaRequirement>,
        cancellation: CancellationToken,
    ) -> ToolJavaFuture<'a>;

    fn run(&self, request: ToolRunRequest) -> ToolRunnerFuture<'_>;
}
