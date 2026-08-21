//! Deterministic install planning and transaction-safe create-only execution.
//!
//! Installation owns the artifact-acquisition port but never owns an HTTP client or cache. The
//! service layer adapts this port to the verified artifact pipeline.

mod acquisition;
mod archive;
mod error;
mod executor;
mod generated;
mod path;
mod plan;
mod processor;
mod request;

pub use acquisition::{AcquiredArtifact, AcquisitionDisposition, ArtifactAcquirer};
pub use archive::{extract_managed_tar_gz, extract_managed_zip};
pub use executor::InstallExecutor;
pub use plan::{
    INSTALL_PLAN_VERSION, InstallPlan, MAX_INSTALL_ARTIFACTS, Materialization,
    MaterializationScope, NativeExtraction, PlannedArtifact, PlannedInstance,
};
pub use processor::{
    InstallToolRunner, ProcessorExpansionContext, SelectedToolJava, ToolJavaFuture, ToolRunRequest,
    ToolRunResult, ToolRunnerFuture, expand_processor_arguments,
};
pub use request::{ComponentInstallRequest, InstallRequest};
