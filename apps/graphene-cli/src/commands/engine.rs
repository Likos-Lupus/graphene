use crate::cli::{EngineArgs, EngineCommand};
use crate::commands::{await_operation, progress_enabled};
use crate::context::AppContext;
use crate::output::Rendered;
use graphene::GrapheneError;
use serde_json::json;
use std::time::Duration;

pub async fn dispatch(context: &AppContext, args: EngineArgs) -> Result<Rendered, GrapheneError> {
    match args.command {
        EngineCommand::Info => Ok(info(context)),
        EngineCommand::Synthetic { steps, delay_ms } => synthetic(context, steps, delay_ms).await,
    }
}

fn info(context: &AppContext) -> Rendered {
    let platform = context.engine.platform();
    let data_root = context.engine.data_root().display().to_string();
    let os = format!("{:?}", platform.os);
    let architecture = format!("{:?}", platform.architecture);
    let human = vec![
        format!("data_root: {data_root}"),
        format!("os: {os}"),
        format!("architecture: {architecture}"),
    ];
    
    Rendered::new(
        human,
        json!({
            "data_root": data_root,
            "os": os,
            "architecture": architecture,
        }),
    )
}

async fn synthetic(
    context: &AppContext,
    steps: u64,
    delay_ms: u64,
) -> Result<Rendered, GrapheneError> {
    let operation =
        context
            .engine
            .operations()
            .synthetic(None, steps, Duration::from_millis(delay_ms));
    let handle = operation.operation();
    let observed = handle.clone();
    let collector = tokio::spawn(async move {
        let stream = observed.subscribe();
        let mut kinds = Vec::new();
        while let Some(event) = stream.next().await {
            kinds.push(serde_json::to_value(&event.kind).unwrap_or(serde_json::Value::Null));
        }
        kinds
    });

    await_operation(handle, operation.await_result(), progress_enabled(context)).await?;
    let events = collector.await.unwrap_or_default();
    let human = vec![format!(
        "synthetic operation completed ({} events)",
        events.len()
    )];
    
    Ok(Rendered::new(human, json!({ "events": events })))
}
