use crate::cli::{ContentArgs, ContentCommand};
use crate::commands::{await_operation, parse_instance, provider_id};
use crate::context::AppContext;
use crate::output::Rendered;
use graphene::{
    ContentActionRequest, ContentMutationRequest, ContentSearchQuery, ContentVersionRef,
    GrapheneError, ReleaseChannelPolicy,
};
use serde_json::json;

pub async fn dispatch(context: &AppContext, args: ContentArgs) -> Result<Rendered, GrapheneError> {
    match args.command {
        ContentCommand::Scan {
            instance_id,
            hashes,
        } => scan(context, &instance_id, hashes).await,

        ContentCommand::Search {
            provider,
            query,
            minecraft,
            loader,
            limit,
        } => search(context, provider, &query, minecraft, loader, limit).await,

        ContentCommand::Recognize { instance_id } => recognize(context, &instance_id).await,

        ContentCommand::Install {
            instance_id,
            provider,
            project,
            version,
            execute,
        } => install(context, &instance_id, provider, &project, &version, execute).await,

        ContentCommand::Update {
            instance_id,
            execute,
        } => update(context, &instance_id, execute).await,
    }
}

async fn scan(
    context: &AppContext,
    instance_id: &str,
    hashes: bool,
) -> Result<Rendered, GrapheneError> {
    let id = parse_instance(instance_id)?;
    let operation = context.engine.content().scan(id, hashes);
    let handle = operation.operation();
    let inventory = await_operation(context, handle, operation.await_result()).await?;
    let human = vec![
        format!("files: {}", inventory.files.len()),
        format!("duplicate_mod_ids: {}", inventory.duplicate_mod_ids.len()),
        format!("fingerprint: {}", inventory.fingerprint.digest()),
    ];
    Ok(Rendered::new(human, Rendered::value(&inventory)))
}

async fn search(
    context: &AppContext,
    provider: crate::cli::ProviderArg,
    query: &str,
    minecraft: Option<String>,
    loader: Option<crate::cli::LoaderArg>,
    limit: u32,
) -> Result<Rendered, GrapheneError> {
    let query = ContentSearchQuery {
        query: query.to_owned(),
        minecraft_version: minecraft,
        loader: loader.map(crate::commands::loader_kind),
        offset: 0,
        limit,
    };
    let operation = context
        .engine
        .content()
        .search(&provider_id(provider), &query);
    let handle = operation.operation();
    let page = await_operation(context, handle, operation.await_result()).await?;
    let human = page
        .hits
        .iter()
        .map(|hit| format!("{}  {}  {}", hit.project_ref, hit.title, hit.summary))
        .collect();
    Ok(Rendered::new(human, Rendered::value(&page)))
}

async fn recognize(context: &AppContext, instance_id: &str) -> Result<Rendered, GrapheneError> {
    let id = parse_instance(instance_id)?;
    let operation = context.engine.content().recognize(id);
    let handle = operation.operation();
    let result = await_operation(context, handle, operation.await_result()).await?;
    let value = Rendered::value(&result);
    Ok(Rendered::new(pretty_lines(&value), value))
}

async fn install(
    context: &AppContext,
    instance_id: &str,
    provider: crate::cli::ProviderArg,
    project: &str,
    version: &str,
    execute: bool,
) -> Result<Rendered, GrapheneError> {
    let id = parse_instance(instance_id)?;
    let version_ref = ContentVersionRef::new(provider_id(provider), project, version)?;
    let request = ContentMutationRequest::new(
        id,
        vec![ContentActionRequest::InstallExactVersion(version_ref)],
    );
    let operation = context.engine.content().plan(&request);
    let handle = operation.operation();
    let plan = await_operation(context, handle, operation.await_result()).await?;

    if !execute {
        return Ok(Rendered::new(
            pretty_lines(&Rendered::value(&plan)),
            Rendered::value(&plan),
        ));
    }

    let operation = context.engine.content().execute(plan);
    let handle = operation.operation();
    let result = await_operation(context, handle, operation.await_result()).await?;

    Ok(execution_rendered(&result))
}

async fn update(
    context: &AppContext,
    instance_id: &str,
    execute: bool,
) -> Result<Rendered, GrapheneError> {
    let id = parse_instance(instance_id)?;
    let operation = context
        .engine
        .content()
        .plan_updates(id, ReleaseChannelPolicy::default());
    let handle = operation.operation();
    let plan = await_operation(context, handle, operation.await_result()).await?;

    if !execute {
        return Ok(Rendered::new(
            pretty_lines(&Rendered::value(&plan)),
            Rendered::value(&plan),
        ));
    }

    let operation = context.engine.content().execute(plan);
    let handle = operation.operation();
    let result = await_operation(context, handle, operation.await_result()).await?;

    Ok(execution_rendered(&result))
}

fn execution_rendered(result: &graphene::ContentMutationResult) -> Rendered {
    let human = vec![format!(
        "modified_entries: {}",
        result.modified_entry_ids.len()
    )];
    let json = json!({
        "modified_entry_ids": result
            .modified_entry_ids
            .iter()
            .map(|entry| entry.to_string())
            .collect::<Vec<_>>(),
        "committed_fingerprint": result.committed_fingerprint.to_string(),
    });

    Rendered::new(human, json)
}

fn pretty_lines(value: &serde_json::Value) -> Vec<String> {
    serde_json::to_string_pretty(value)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}
