use crate::cli::{InstallArgs, InstallCommand};
use crate::commands::{await_operation, loader_selection, progress_enabled};
use crate::context::AppContext;
use crate::output::Rendered;
use graphene::{ComponentInstallRequest, GrapheneError, InstallPlanOperation, InstallRequest};

pub async fn dispatch(context: &AppContext, args: InstallArgs) -> Result<Rendered, GrapheneError> {
    match args.command {
        InstallCommand::Vanilla { minecraft, name } => {
            let request = InstallRequest::new(name, minecraft)?;
            run_plan(context, context.engine.install().plan(request)).await
        }

        InstallCommand::Loader {
            minecraft,
            loader,
            loader_version,
            name,
        } => {
            let selection = loader_selection(loader, loader_version.as_deref())?;
            let request = ComponentInstallRequest::new(name, minecraft, selection)?;
            run_plan(context, context.engine.install().plan_components(request)).await
        }
    }
}

async fn run_plan(
    context: &AppContext,
    operation: InstallPlanOperation,
) -> Result<Rendered, GrapheneError> {
    let handle = operation.operation();
    let plan = await_operation(handle, operation.await_result(), progress_enabled(context)).await?;

    let execute = context.engine.install().execute(plan);
    let handle = execute.operation();
    let committed =
        await_operation(handle, execute.await_result(), progress_enabled(context)).await?;

    let human = vec![
        format!("instance: {}", committed.descriptor.instance_id),
        format!("name: {}", committed.descriptor.display_name),
        format!("minecraft: {}", committed.receipt.requested_version),
        format!("status: {:?}", committed.status),
    ];

    Ok(Rendered::new(human, Rendered::value(&committed)))
}
