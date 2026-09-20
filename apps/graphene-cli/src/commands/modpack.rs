use crate::cli::{ModpackArgs, ModpackCommand};
use crate::commands::{await_operation, parse_instance, progress_enabled};
use crate::context::AppContext;
use crate::output::Rendered;
use graphene::{
    ExportEmbeddingPolicy, GrapheneError, ModpackExportRequest, ModpackImportRequest,
    NewInstanceSpec, OptionalSelectionPolicy, PackSource,
};
use serde_json::json;
use std::path::PathBuf;

pub async fn dispatch(context: &AppContext, args: ModpackArgs) -> Result<Rendered, GrapheneError> {
    match args.command {
        ModpackCommand::Inspect { source } => inspect(context, &source).await,
        
        ModpackCommand::Import {
            source,
            name,
            execute,
        } => import(context, &source, &name, execute).await,
        
        ModpackCommand::Export {
            instance_id,
            name,
            out,
        } => export(context, &instance_id, &name, out).await,
    }
}

fn pack_source(source: &str) -> PackSource {
    if source.starts_with("https://") {
        PackSource::HttpsUrl(source.to_owned())
    } else {
        PackSource::LocalFile(PathBuf::from(source))
    }
}

async fn inspect(context: &AppContext, source: &str) -> Result<Rendered, GrapheneError> {
    let operation = context.engine.modpacks().inspect(pack_source(source));
    let handle = operation.operation();
    let inspection =
        await_operation(handle, operation.await_result(), progress_enabled(context)).await?;
    let value = Rendered::value(&inspection);
    let human = vec![
        format!("required_files: {}", value["required_file_count"]),
        format!("embedded_mods: {}", value["embedded_mod_count"]),
        format!(
            "requires_provider_resolution: {}",
            value["requires_provider_resolution"]
        ),
    ];
    
    Ok(Rendered::new(human, value))
}

async fn import(
    context: &AppContext,
    source: &str,
    name: &str,
    execute: bool,
) -> Result<Rendered, GrapheneError> {
    let operation = context.engine.modpacks().inspect(pack_source(source));
    let handle = operation.operation();
    let inspection =
        await_operation(handle, operation.await_result(), progress_enabled(context)).await?;

    let request = ModpackImportRequest {
        snapshot: inspection.snapshot().clone(),
        target: NewInstanceSpec::new(name)?,
        optional_policy: OptionalSelectionPolicy::RequiredOnly,
    };
    let operation = context.engine.modpacks().plan_import(request);
    let handle = operation.operation();
    let plan = await_operation(handle, operation.await_result(), progress_enabled(context)).await?;

    if !execute {
        return Ok(Rendered::new(
            vec![format!(
                "import plan ready (schema {}, format {:?})",
                plan.schema_version, plan.pack_format
            )],
            json!({
                "planned": true,
                "schema_version": plan.schema_version,
                "fingerprint": plan.fingerprint.to_string(),
            }),
        ));
    }

    let operation = context.engine.modpacks().execute_import(plan);
    let handle = operation.operation();
    let committed =
        await_operation(handle, operation.await_result(), progress_enabled(context)).await?;
    let human = vec![
        format!("instance: {}", committed.descriptor.instance_id),
        format!("name: {}", committed.descriptor.display_name),
        format!("minecraft: {}", committed.receipt.requested_version),
    ];
    
    Ok(Rendered::new(human, Rendered::value(&committed)))
}

async fn export(
    context: &AppContext,
    instance_id: &str,
    name: &str,
    out: PathBuf,
) -> Result<Rendered, GrapheneError> {
    let id = parse_instance(instance_id)?;
    let request = ModpackExportRequest::new(
        id,
        name,
        None,
        None,
        out,
        ExportEmbeddingPolicy::ReferenceOnly,
    )?;
    let operation = context.engine.modpacks().plan_export(request);
    let handle = operation.operation();
    let plan = await_operation(handle, operation.await_result(), progress_enabled(context)).await?;

    let operation = context.engine.modpacks().execute_export(plan);
    let handle = operation.operation();
    let result =
        await_operation(handle, operation.await_result(), progress_enabled(context)).await?;

    let human = vec![
        format!("output: {}", result.output.display()),
        format!("archive_size: {}", result.archive_size),
        format!(
            "referenced_managed_files: {}",
            result.referenced_managed_files
        ),
    ];
    
    Ok(Rendered::new(
        human,
        json!({
            "output": result.output.display().to_string(),
            "archive_sha256": result.archive_sha256,
            "archive_size": result.archive_size,
            "referenced_managed_files": result.referenced_managed_files,
            "embedded_objects": result.embedded_objects,
            "seed_files": result.seed_files,
        }),
    ))
}
