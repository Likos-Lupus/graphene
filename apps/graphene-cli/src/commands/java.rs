use crate::cli::{JavaArgs, JavaCommand};
use crate::commands::{await_operation, parse_instance};
use crate::context::AppContext;
use crate::output::Rendered;
use graphene::{GrapheneError, JavaRequirement};

pub async fn dispatch(context: &AppContext, args: JavaArgs) -> Result<Rendered, GrapheneError> {
    match args.command {
        JavaCommand::List => list(context).await,

        JavaCommand::Ensure { instance_id } => ensure(context, &instance_id).await,

        JavaCommand::Install { major } => install(context, major).await,
    }
}

async fn list(context: &AppContext) -> Result<Rendered, GrapheneError> {
    let runtimes = context.engine.java().managed_runtimes().await?;
    let human = runtimes
        .iter()
        .map(|runtime| {
            format!(
                "{}  major={}  {}  {:?}/{:?}",
                runtime.id,
                runtime.major_version,
                runtime.version,
                runtime.os,
                runtime.architecture
            )
        })
        .collect();

    Ok(Rendered::new(human, Rendered::value(&runtimes)))
}

async fn ensure(context: &AppContext, instance_id: &str) -> Result<Rendered, GrapheneError> {
    let id = parse_instance(instance_id)?;
    let operation = context.engine.java().ensure_for_instance(id, None);
    let handle = operation.operation();
    let runtime = await_operation(context, handle, operation.await_result()).await?;
    let human = vec![
        format!("major: {}", runtime.major_version),
        format!("version: {}", runtime.version),
        format!("executable: {}", runtime.executable.display()),
    ];

    Ok(Rendered::new(human, Rendered::value(&runtime)))
}

async fn install(context: &AppContext, major: u32) -> Result<Rendered, GrapheneError> {
    let requirement = JavaRequirement {
        major_version: major,
        component_hint: None,
    };
    let operation = context.engine.java().install_managed(requirement);
    let handle = operation.operation();
    let runtime = await_operation(context, handle, operation.await_result()).await?;
    let human = vec![
        format!("major: {}", runtime.major_version),
        format!("version: {}", runtime.version),
    ];

    Ok(Rendered::new(human, Rendered::value(&runtime)))
}
