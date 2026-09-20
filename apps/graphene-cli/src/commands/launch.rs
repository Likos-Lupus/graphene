use crate::cli::{LaunchArgs, LaunchCommand};
use crate::commands::{await_operation, parse_account, parse_instance, progress_enabled};
use crate::context::AppContext;
use crate::error::cancelled;
use crate::output::Rendered;
use graphene::{
    ErrorCode, ErrorKind, GameEvent, GrapheneError, LaunchPlan, LaunchRequest, LaunchSession,
};
use graphene_reference_host_support::HostRunRegistry;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

pub async fn dispatch(context: &AppContext, args: LaunchArgs) -> Result<Rendered, GrapheneError> {
    match args.command {
        LaunchCommand::Plan {
            instance_id,
            account,
        } => plan(context, &instance_id, account.as_deref()).await,

        LaunchCommand::Run {
            instance_id,
            account,
        } => run(context, &instance_id, account.as_deref()).await,

        LaunchCommand::Kill { pid } => kill(pid),
    }
}

async fn plan(
    context: &AppContext,
    instance_id: &str,
    account: Option<&str>,
) -> Result<Rendered, GrapheneError> {
    let plan = build_plan(context, instance_id, account).await?;
    Ok(redacted(context, &plan))
}

async fn run(
    context: &AppContext,
    instance_id: &str,
    account: Option<&str>,
) -> Result<Rendered, GrapheneError> {
    let plan = build_plan(context, instance_id, account).await?;
    let game = context.engine.launch().execute(plan)?;
    let registry = HostRunRegistry::new(256, Duration::from_secs(1));
    let run_id = registry.insert(Arc::new(game));
    let mut events = registry.subscribe();

    let outcome = loop {
        tokio::select! {
            result = registry.wait(&run_id) => break result,
            signal = tokio::signal::ctrl_c() => {
                let _ = signal;
                let _ = registry.kill(&run_id).await;
                let _ = registry.wait(&run_id).await;
                break Err(cancelled());
            }
            event = events.recv() => {
                match event {
                    Ok(envelope) => render_game_event(context, &envelope.event),
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(dropped)) => {
                        eprintln!("dropped {dropped} game events");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {}
                }
            }
        }
    };

    let _ = registry.remove(&run_id);
    let exit = outcome?;
    let human = vec![
        format!("success: {}", exit.success),
        format!("exit_code: {:?}", exit.code),
        format!("killed: {}", exit.killed),
    ];

    Ok(Rendered::new(
        human,
        json!({ "success": exit.success, "exit_code": exit.code, "killed": exit.killed }),
    ))
}

async fn build_plan(
    context: &AppContext,
    instance_id: &str,
    account: Option<&str>,
) -> Result<LaunchPlan, GrapheneError> {
    let id = parse_instance(instance_id)?;
    let account_id = account.ok_or_else(|| {
        GrapheneError::new(
            ErrorCode::LaunchPlanInvalid,
            ErrorKind::Launch,
            "launch requires --account for its ephemeral session",
        )
    })?;
    let account_id = parse_account(account_id)?;

    let session = context.engine.accounts().launch_session(account_id);
    let handle = session.operation();
    let session: LaunchSession =
        await_operation(handle, session.await_result(), progress_enabled(context)).await?;

    let request = LaunchRequest::new(id, session);
    context.engine.launch().plan(request).await
}

fn redacted(context: &AppContext, plan: &LaunchPlan) -> Rendered {
    let snapshot = plan.redacted_snapshot(context.engine.data_root());
    let human = vec![
        format!("instance_id: {}", snapshot.instance_id),
        format!("java: {}", snapshot.java),
        format!("main_class: {}", snapshot.main_class),
        format!("classpath_entries: {}", snapshot.classpath.len()),
        format!("jvm_args: {}", snapshot.jvm_args.len()),
        format!("game_args: {}", snapshot.game_args.len()),
    ];

    Rendered::new(human, Rendered::value(&snapshot))
}

fn render_game_event(context: &AppContext, event: &GameEvent) {
    if context.output.is_json() {
        eprintln!(
            "{}",
            json!({ "event": "game", "kind": game_event_kind(event), "text": game_event_text(event) })
        );
        return;
    }

    match event {
        GameEvent::Started { pid } => eprintln!("game started (pid {pid})"),
        GameEvent::Stdout { text, .. } => print!("{text}"),
        GameEvent::Stderr { text, .. } => eprint!("{text}"),
        GameEvent::Exited { exit } => eprintln!("game exited (success {})", exit.success),
        GameEvent::Failed { code } => eprintln!("game failed ({code})"),
        _ => {}
    }
}

fn game_event_kind(event: &GameEvent) -> &'static str {
    match event {
        GameEvent::Started { .. } => "started",
        GameEvent::Stdout { .. } => "stdout",
        GameEvent::Stderr { .. } => "stderr",
        GameEvent::Exited { .. } => "exited",
        GameEvent::Failed { .. } => "failed",
        _ => "unknown",
    }
}

fn game_event_text(event: &GameEvent) -> Option<&str> {
    match event {
        GameEvent::Stdout { text, .. } | GameEvent::Stderr { text, .. } => Some(text),
        _ => None,
    }
}

fn kill(pid: u32) -> Result<Rendered, GrapheneError> {
    #[cfg(unix)]
    {
        // SAFETY: `pid` is validated as a non-negative integer; `kill` has no memory safety impact.
        let result = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        if result == 0 {
            Ok(Rendered::new(
                vec![format!("sent SIGTERM to pid {pid}")],
                json!({ "pid": pid }),
            ))
        } else {
            Err(GrapheneError::new(
                ErrorCode::LaunchProcessIoFailed,
                ErrorKind::Launch,
                "failed to signal the requested process",
            )
            .with_context("pid", pid.to_string()))
        }
    }

    #[cfg(not(unix))]
    {
        let _ = pid;
        Err(GrapheneError::new(
            ErrorCode::PlatformUnsupported,
            ErrorKind::Platform,
            "launch kill is unsupported on this platform",
        ))
    }
}
