use crate::dto::parse_instance_id;
use crate::error::HostError;
use crate::state::TauriAppState;
use graphene::{
    ExportEmbeddingPolicy, ModpackExportRequest, ModpackImportRequest, NewInstanceSpec,
    OptionalSelectionPolicy, PackInspection, PackSource,
};
use serde_json::{Value, json};
use std::path::PathBuf;
use tauri::State;

fn pack_source_of(source: &str) -> PackSource {
    if source.starts_with("https://") {
        PackSource::HttpsUrl(source.to_owned())
    } else {
        PackSource::LocalFile(PathBuf::from(source))
    }
}

pub(crate) async fn inspect(
    state: &TauriAppState,
    source: &str,
) -> Result<PackInspection, HostError> {
    let operation = state.engine().modpacks().inspect(pack_source_of(source));
    operation.await_result().await.map_err(HostError::from)
}

pub(crate) async fn import(
    state: &TauriAppState,
    source: &str,
    name: String,
    execute: bool,
) -> Result<Value, HostError> {
    let inspection = inspect(state, source).await?;
    let request = ModpackImportRequest {
        snapshot: inspection.snapshot().clone(),
        target: NewInstanceSpec::new(name)?,
        optional_policy: OptionalSelectionPolicy::RequiredOnly,
    };
    let plan = state
        .engine()
        .modpacks()
        .plan_import(request)
        .await_result()
        .await?;

    if !execute {
        return Ok(json!({
            "planned": true,
            "schema_version": plan.schema_version,
            "fingerprint": plan.fingerprint.to_string(),
        }));
    }

    let committed = state
        .engine()
        .modpacks()
        .execute_import(plan)
        .await_result()
        .await?;

    Ok(json!({
        "planned": false,
        "instance_id": committed.descriptor.instance_id.to_string(),
        "name": committed.descriptor.display_name,
        "minecraft": committed.receipt.requested_version,
    }))
}

pub(crate) async fn export(
    state: &TauriAppState,
    instance_id: &str,
    name: String,
    out: PathBuf,
) -> Result<Value, HostError> {
    let request = ModpackExportRequest::new(
        parse_instance_id(instance_id)?,
        name,
        None,
        None,
        out,
        ExportEmbeddingPolicy::ReferenceOnly,
    )?;
    let plan = state
        .engine()
        .modpacks()
        .plan_export(request)
        .await_result()
        .await?;
    let result = state
        .engine()
        .modpacks()
        .execute_export(plan)
        .await_result()
        .await?;

    Ok(json!({
        "output": result.output.display().to_string(),
        "archive_sha256": result.archive_sha256,
        "archive_size": result.archive_size,
        "referenced_managed_files": result.referenced_managed_files,
        "embedded_objects": result.embedded_objects,
        "seed_files": result.seed_files,
    }))
}

#[tauri::command]
pub async fn modpack_inspect(
    state: State<'_, TauriAppState>,
    source: String,
) -> Result<PackInspection, HostError> {
    inspect(state.inner(), &source).await
}

#[tauri::command]
pub async fn modpack_import(
    state: State<'_, TauriAppState>,
    source: String,
    name: String,
    execute: bool,
) -> Result<Value, HostError> {
    import(state.inner(), &source, name, execute).await
}

#[tauri::command]
pub async fn modpack_export(
    state: State<'_, TauriAppState>,
    instance_id: String,
    name: String,
    out: String,
) -> Result<Value, HostError> {
    export(state.inner(), &instance_id, name, PathBuf::from(out)).await
}
