use crate::cli::{AccountArgs, AccountCommand};
use crate::commands::{await_operation, parse_account, progress_enabled};
use crate::context::AppContext;
use crate::output::Rendered;
use graphene::{GrapheneError, MicrosoftLoginOperation, OfflineAccountSpec};

pub async fn dispatch(context: &AppContext, args: AccountArgs) -> Result<Rendered, GrapheneError> {
    match args.command {
        AccountCommand::List => list(context).await,
        AccountCommand::OfflineAdd { name } => offline_add(context, &name).await,
        AccountCommand::MicrosoftLogin => microsoft_login(context).await,
        AccountCommand::Reauth { account_id } => reauth(context, &account_id).await,
        AccountCommand::Remove { account_id } => remove(context, &account_id).await,
    }
}

async fn list(context: &AppContext) -> Result<Rendered, GrapheneError> {
    let accounts = context.engine.accounts().list().await?;
    let human = accounts
        .iter()
        .map(|account| {
            format!(
                "{}  {:?}  {:?}  {}",
                account.id, account.kind, account.state, account.profile.display_name
            )
        })
        .collect();
    Ok(Rendered::new(human, Rendered::value(&accounts)))
}

async fn offline_add(context: &AppContext, name: &str) -> Result<Rendered, GrapheneError> {
    let account = context
        .engine
        .accounts()
        .create_offline(OfflineAccountSpec::new(name))
        .await?;
    let human = vec![
        format!("id: {}", account.id),
        format!("display_name: {}", account.profile.display_name),
    ];
    Ok(Rendered::new(human, Rendered::value(&account)))
}

async fn microsoft_login(context: &AppContext) -> Result<Rendered, GrapheneError> {
    let operation = context.engine.accounts().begin_microsoft_login().await?;
    finish_login(context, operation).await
}

async fn reauth(context: &AppContext, account_id: &str) -> Result<Rendered, GrapheneError> {
    let id = parse_account(account_id)?;
    let operation = context.engine.accounts().reauthenticate(id).await?;
    finish_login(context, operation).await
}

async fn finish_login(
    context: &AppContext,
    operation: MicrosoftLoginOperation,
) -> Result<Rendered, GrapheneError> {
    crate::interaction::emit(context.output, operation.interaction());
    
    let handle = operation.operation();
    let account =
        await_operation(handle, operation.await_result(), progress_enabled(context)).await?;
    let human = vec![
        format!("id: {}", account.id),
        format!("display_name: {}", account.profile.display_name),
        format!("state: {:?}", account.state),
    ];
    
    Ok(Rendered::new(human, Rendered::value(&account)))
}

async fn remove(context: &AppContext, account_id: &str) -> Result<Rendered, GrapheneError> {
    let id = parse_account(account_id)?;
    context.engine.accounts().remove(id).await?;
    Ok(Rendered::new(
        vec![format!("removed account {account_id}")],
        serde_json::json!({ "account_id": account_id }),
    ))
}
