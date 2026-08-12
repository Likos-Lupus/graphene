mod event;
mod output;
mod running;

pub use event::{GameEvent, GameEventStream, GameExit};
pub use output::MAX_PROCESS_CHUNK_BYTES;
pub use running::RunningGame;

use self::{
    output::{OutputKind, drain_output},
    running::monitor_process,
};
use crate::{
    error::launch_error,
    plan::{LaunchArgument, LaunchPlan},
};
use graphene_core::{ErrorCode, Result};
use graphene_platform::{PlatformProcess, ProcessSpec, normalize_process_path};
use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{Arc, Mutex, atomic::AtomicU64},
};
use tokio::sync::{Mutex as AsyncMutex, Notify, mpsc};

pub const DEFAULT_GAME_EVENT_CAPACITY: usize = 256;

fn process_spec(plan: &LaunchPlan) -> Result<ProcessSpec> {
    plan.validate()?;
    let classpath = materialize_classpath(&plan.classpath, plan.classpath_separator)?;
    let mut argv = Vec::<OsString>::new();

    for argument in &plan.jvm_args {
        argv.push(materialize_argument(
            argument,
            &classpath,
            plan.classpath_separator,
        ));
    }

    argv.push(OsString::from(&plan.main_class));
    for argument in &plan.game_args {
        argv.push(materialize_argument(
            argument,
            &classpath,
            plan.classpath_separator,
        ));
    }

    let mut spec = ProcessSpec::new(&plan.java.executable)
        .args(argv)
        .current_dir(&plan.working_directory);
    for (key, value) in &plan.environment.values {
        spec = spec.env(key, value);
    }

    Ok(spec)
}

fn materialize_argument(argument: &LaunchArgument, classpath: &str, separator: char) -> OsString {
    match argument {
        LaunchArgument::Plain(value) => OsString::from(value),
        LaunchArgument::Secret(value) => OsString::from(value.expose_secret()),
        LaunchArgument::Classpath => OsString::from(classpath),
        LaunchArgument::ClasspathSeparator => OsString::from(separator.to_string()),
    }
}

fn materialize_classpath(classpath: &[PathBuf], separator: char) -> Result<String> {
    let mut entries = Vec::with_capacity(classpath.len());
    for path in classpath {
        let value = normalize_process_path(path)
            .into_os_string()
            .into_string()
            .map_err(|_| {
                launch_error(
                    ErrorCode::LaunchPlanInvalid,
                    "launch path is not valid UTF-8",
                )
            })?;
        entries.push(value);
    }

    Ok(entries.join(&separator.to_string()))
}

/// Spawns Java directly. Secrets are exposed only while constructing this child-process argv.
pub fn execute(plan: LaunchPlan, event_capacity: usize) -> Result<RunningGame> {
    if !(4..=65_536).contains(&event_capacity) {
        return Err(launch_error(
            ErrorCode::LaunchPlanInvalid,
            "game event capacity must be between 4 and 65536",
        ));
    }

    let spec = process_spec(&plan)?;
    let mut process = PlatformProcess::spawn(&spec).map_err(|source| {
        launch_error(
            ErrorCode::LaunchProcessSpawnFailed,
            "failed to spawn Java process",
        )
        .with_source(source)
    })?;
    let pid = process.id().ok_or_else(|| {
        launch_error(
            ErrorCode::LaunchProcessSpawnFailed,
            "spawned Java process has no process ID",
        )
    })?;
    let stdout = process.take_stdout().ok_or_else(|| {
        launch_error(
            ErrorCode::LaunchProcessIoFailed,
            "spawned Java process has no stdout pipe",
        )
    })?;
    let stderr = process.take_stderr().ok_or_else(|| {
        launch_error(
            ErrorCode::LaunchProcessIoFailed,
            "spawned Java process has no stderr pipe",
        )
    })?;
    let (event_sender, event_receiver) = mpsc::channel(event_capacity);
    let (control_sender, control_receiver) = mpsc::channel(1);
    let terminal = Arc::new(AsyncMutex::new(None));
    let terminal_notify = Arc::new(Notify::new());
    let dropped_output = Arc::new(AtomicU64::new(0));

    let _ = event_sender.try_send(GameEvent::Started { pid });
    let stdout_task = tokio::spawn(drain_output(
        stdout,
        OutputKind::Stdout,
        event_sender.clone(),
        Arc::clone(&dropped_output),
    ));
    let stderr_task = tokio::spawn(drain_output(
        stderr,
        OutputKind::Stderr,
        event_sender.clone(),
        Arc::clone(&dropped_output),
    ));

    tokio::spawn(monitor_process(
        process,
        control_receiver,
        stdout_task,
        stderr_task,
        event_sender,
        Arc::clone(&terminal),
        Arc::clone(&terminal_notify),
    ));

    Ok(RunningGame {
        pid,
        events: Mutex::new(Some(event_receiver)),
        control: control_sender,
        terminal,
        terminal_notify,
        dropped_output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphene_platform::classpath_separator;

    #[test]
    fn classpath_materialization_preserves_order_and_explicit_separator() {
        let paths = vec![PathBuf::from("first.jar"), PathBuf::from("second.jar")];
        assert_eq!(
            materialize_classpath(&paths, ':').expect("unix classpath"),
            "first.jar:second.jar"
        );
        assert_eq!(
            materialize_classpath(&paths, ';').expect("windows classpath"),
            "first.jar;second.jar"
        );
        assert_eq!(classpath_separator(), if cfg!(windows) { ';' } else { ':' });
    }
}
