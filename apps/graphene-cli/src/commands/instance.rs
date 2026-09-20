use crate::cli::{InstanceArgs, InstanceCommand, InstanceConfigArgs, InstanceConfigCommand};
use crate::commands::{await_operation, parse_instance, progress_enabled};
use crate::context::AppContext;
use crate::output::Rendered;
use graphene::{CloneRequest, DeleteOptions, GrapheneError, RepairOptions, VerificationMode};
use serde_json::Value;

pub async fn dispatch(context: &AppContext, args: InstanceArgs) -> Result<Rendered, GrapheneError> {
    match args.command {
        InstanceCommand::List => list(context).await,

        InstanceCommand::Get { instance_id } => get(context, &instance_id).await,

        InstanceCommand::Verify { instance_id, full } => verify(context, &instance_id, full).await,

        InstanceCommand::Repair {
            instance_id,
            execute,
            full,
        } => repair(context, &instance_id, execute, full).await,

        InstanceCommand::Clone { instance_id, name } => clone(context, &instance_id, &name).await,

        InstanceCommand::Delete { instance_id } => delete(context, &instance_id).await,

        InstanceCommand::Config(args) => config(context, args).await,
    }
}

async fn list(context: &AppContext) -> Result<Rendered, GrapheneError> {
    let entries = context.engine.instances().list().await?;
    let human = entries
        .iter()
        .map(|entry| {
            format!(
                "{}  {:?}  {}  mc={}",
                entry.instance_id,
                entry.status,
                entry.display_name.clone().unwrap_or_default(),
                entry.minecraft_version.clone().unwrap_or_default(),
            )
        })
        .collect();

    Ok(Rendered::new(human, Rendered::value(&entries)))
}

async fn get(context: &AppContext, instance_id: &str) -> Result<Rendered, GrapheneError> {
    let id = parse_instance(instance_id)?;
    let committed = context.engine.instances().get(id).await?;
    let human = vec![
        format!("id: {}", committed.descriptor.instance_id),
        format!("name: {}", committed.descriptor.display_name),
        format!("status: {:?}", committed.status),
        format!("minecraft: {}", committed.receipt.requested_version),
        format!("main_class: {}", committed.receipt.main_class),
    ];

    Ok(Rendered::new(human, Rendered::value(&committed)))
}

async fn verify(
    context: &AppContext,
    instance_id: &str,
    full: bool,
) -> Result<Rendered, GrapheneError> {
    let id = parse_instance(instance_id)?;
    let mode = if full {
        VerificationMode::Full
    } else {
        VerificationMode::Quick
    };
    let operation = context.engine.instances().verify(id, mode);
    let handle = operation.operation();
    let report =
        await_operation(handle, operation.await_result(), progress_enabled(context)).await?;
    let human = vec![
        format!("healthy: {}", report.is_healthy()),
        format!("repairability: {:?}", report.repairability),
        format!("summary: {}", report.summary),
    ];

    Ok(Rendered::new(human, Rendered::value(&report)))
}

async fn repair(
    context: &AppContext,
    instance_id: &str,
    execute: bool,
    full: bool,
) -> Result<Rendered, GrapheneError> {
    let id = parse_instance(instance_id)?;
    let options = RepairOptions {
        verification_mode: if full {
            VerificationMode::Full
        } else {
            VerificationMode::Quick
        },
    };
    let plan = context.engine.instances().plan_repair(id, options).await?;

    if !execute {
        let human = vec![
            format!("noop: {}", plan.is_noop()),
            format!("actions: {}", plan.ordered_actions.len()),
            format!("requires_network: {}", plan.requires_network),
            format!("requires_tool_java: {}", plan.requires_tool_java),
        ];
        return Ok(Rendered::new(human, Rendered::value(&plan)));
    }

    let operation = context.engine.instances().execute_repair(plan);
    let handle = operation.operation();
    let result =
        await_operation(handle, operation.await_result(), progress_enabled(context)).await?;
    let human = vec![
        format!("executed_actions: {}", result.executed_actions_count),
        format!("healthy: {}", result.post_verify_report.is_healthy()),
    ];

    Ok(Rendered::new(human, Rendered::value(&result)))
}

async fn clone(
    context: &AppContext,
    instance_id: &str,
    name: &str,
) -> Result<Rendered, GrapheneError> {
    let id = parse_instance(instance_id)?;
    let request = CloneRequest::new(name)?;
    let operation = context.engine.instances().clone(id, request);
    let handle = operation.operation();

    await_operation(handle, operation.await_result(), progress_enabled(context)).await?;
    Ok(Rendered::new(
        vec![format!("cloned {instance_id} to new instance '{name}'")],
        serde_json::json!({ "destination_display_name": name }),
    ))
}

async fn delete(context: &AppContext, instance_id: &str) -> Result<Rendered, GrapheneError> {
    let id = parse_instance(instance_id)?;
    let operation = context.engine.instances().delete(id, DeleteOptions {});
    let handle = operation.operation();

    await_operation(handle, operation.await_result(), progress_enabled(context)).await?;
    Ok(Rendered::new(
        vec![format!("deleted instance {instance_id}")],
        serde_json::json!({ "instance_id": instance_id }),
    ))
}

async fn config(context: &AppContext, args: InstanceConfigArgs) -> Result<Rendered, GrapheneError> {
    match args.command {
        InstanceConfigCommand::Global => {
            let global = context.engine.instances().global_defaults().await?;
            let value = Rendered::value(&global);
            Ok(Rendered::new(pretty_lines(&value), value))
        }

        InstanceConfigCommand::Effective { instance_id } => {
            let id = parse_instance(&instance_id)?;
            let effective = context.engine.instances().effective_config(id).await?;
            let value = Rendered::value(&effective);
            Ok(Rendered::new(pretty_lines(&value), value))
        }
    }
}

fn pretty_lines(value: &Value) -> Vec<String> {
    serde_json::to_string_pretty(value)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}
