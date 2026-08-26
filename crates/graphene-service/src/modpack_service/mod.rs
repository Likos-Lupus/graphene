//! Modpack resolution service.
//!
//! Bridges the `graphene-modpack` domain model to the content provider catalog.
//! Resolution takes `PendingProviderFile` entries declared by a normalized modpack
//! and resolves them to exact provider versions through the `ContentProvider` boundary,
//! producing fully-specified `NormalizedPackFile` entries ready for installation planning.

mod export;
mod inspection;
mod operations;
mod planning;
mod resolution;
mod source;

#[allow(unused_imports)]
pub(crate) use export::plan_export;
pub use export::{
    ExportEmbeddingPolicy, ModpackExportPlan, ModpackExportRequest, ModpackExportResult,
};
mod export_execute;
pub use operations::{
    ModpackExecutionOperation, ModpackExportExecutionOperation, ModpackExportPlanOperation,
    ModpackInspectionOperation, ModpackPlanOperation, ModpackService,
};
#[allow(unused_imports)]
pub(crate) use planning::plan_import;
pub use planning::{ModpackImportPlan, ModpackImportRequest};
pub use source::PackSource;
