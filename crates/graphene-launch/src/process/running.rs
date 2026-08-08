use super::event::{GameEvent, GameEventStream, GameExit};
use crate::error::launch_error;
use graphene_core::{ErrorCode, ErrorSummary, GrapheneError, Result};
use graphene_platform::PlatformProcess;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::{Mutex as AsyncMutex, Notify, mpsc, oneshot};

#[derive(Clone)]
pub(super) enum Terminal {
    Exited(GameExit),
    Failed(ErrorSummary),
}

pub(super) enum Control {
    Kill(oneshot::Sender<std::io::Result<()>>),
}

pub struct RunningGame {
    pub(super) pid: u32,
    pub(super) events: Mutex<Option<mpsc::Receiver<GameEvent>>>,
    pub(super) control: mpsc::Sender<Control>,
    pub(super) terminal: Arc<AsyncMutex<Option<Terminal>>>,
    pub(super) terminal_notify: Arc<Notify>,
    pub(super) dropped_output: Arc<AtomicU64>,
}

impl std::fmt::Debug for RunningGame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunningGame")
            .field("pid", &self.pid)
            .field(
                "dropped_output",
                &self.dropped_output.load(Ordering::Relaxed),
            )
            .finish_non_exhaustive()
    }
}

impl RunningGame {
    #[must_use]
    pub const fn pid(&self) -> u32 {
        self.pid
    }

    /// Takes the single bounded event stream. Lifecycle state remains available through `wait`.
    pub fn take_events(&self) -> Option<GameEventStream> {
        let receiver = match self.events.lock() {
            Ok(mut events) => events.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        };
        receiver.map(|receiver| GameEventStream { receiver })
    }

    #[must_use]
    pub fn dropped_output_count(&self) -> u64 {
        self.dropped_output.load(Ordering::Relaxed)
    }

    pub async fn wait(&self) -> Result<GameExit> {
        loop {
            let notified = self.terminal_notify.notified();
            if let Some(terminal) = self.terminal.lock().await.clone() {
                return terminal_result(terminal);
            }

            notified.await;
        }
    }

    pub async fn kill(&self) -> Result<()> {
        if self.terminal.lock().await.is_some() {
            return Ok(());
        }

        let (sender, receiver) = oneshot::channel();
        if self.control.send(Control::Kill(sender)).await.is_err() {
            if self.terminal.lock().await.is_some() {
                return Ok(());
            }

            return Err(launch_error(
                ErrorCode::LaunchProcessIoFailed,
                "game process control channel closed unexpectedly",
            ));
        }

        match receiver.await {
            Ok(result) => result.map_err(|source| {
                launch_error(
                    ErrorCode::LaunchProcessIoFailed,
                    "failed to terminate game process",
                )
                .with_source(source)
            }),
            Err(_) if self.terminal.lock().await.is_some() => Ok(()),
            Err(_) => Err(launch_error(
                ErrorCode::LaunchProcessIoFailed,
                "game process kill acknowledgement was lost",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn kill_is_idempotent_after_terminal_process_control_closes() {
        let (control, receiver) = mpsc::channel(1);
        drop(receiver);
        let game = RunningGame {
            pid: 7,
            events: Mutex::new(None),
            control,
            terminal: Arc::new(AsyncMutex::new(Some(Terminal::Exited(GameExit {
                success: true,
                code: Some(0),
                killed: false,
            })))),
            terminal_notify: Arc::new(Notify::new()),
            dropped_output: Arc::new(AtomicU64::new(0)),
        };

        game.kill().await.expect("terminal kill is idempotent");
    }
}
pub(super) async fn monitor_process(
    mut process: PlatformProcess,
    mut control: mpsc::Receiver<Control>,
    stdout: tokio::task::JoinHandle<std::io::Result<()>>,
    stderr: tokio::task::JoinHandle<std::io::Result<()>>,
    sender: mpsc::Sender<GameEvent>,
    terminal: Arc<AsyncMutex<Option<Terminal>>>,
    notify: Arc<Notify>,
) {
    let mut killed = false;
    let exit_result = loop {
        tokio::select! {
            result = process.wait() => break result,
            command = control.recv() => {
                match command {
                    Some(Control::Kill(reply)) => {
                        let result = process.kill().await;
                        if result.is_ok() { killed = true; }
                        let _ = reply.send(result);
                    }
                    None => break process.wait().await,
                }
            }
        }
    };
    let stdout_result = stdout.await;
    let stderr_result = stderr.await;
    let terminal_value = match exit_result {
        Err(source) => Terminal::Failed(
            launch_error(
                ErrorCode::LaunchProcessIoFailed,
                "failed while waiting for game process",
            )
            .with_source(source)
            .summary(),
        ),
        Ok(exit) => {
            let output_failed = stdout_result.is_err()
                || stderr_result.is_err()
                || stdout_result.as_ref().is_ok_and(|result| result.is_err())
                || stderr_result.as_ref().is_ok_and(|result| result.is_err());
            if output_failed {
                Terminal::Failed(
                    launch_error(
                        ErrorCode::LaunchProcessIoFailed,
                        "failed while draining game process output",
                    )
                    .summary(),
                )
            } else {
                Terminal::Exited(GameExit {
                    success: exit.success,
                    code: exit.code,
                    killed,
                })
            }
        }
    };

    match &terminal_value {
        Terminal::Exited(exit) => {
            let _ = sender.try_send(GameEvent::Exited { exit: *exit });
        }
        Terminal::Failed(error) => {
            let _ = sender.try_send(GameEvent::Failed { code: error.code });
        }
    }

    *terminal.lock().await = Some(terminal_value);
    notify.notify_waiters();
}

fn terminal_result(terminal: Terminal) -> Result<GameExit> {
    match terminal {
        Terminal::Exited(exit) => Ok(exit),
        Terminal::Failed(summary) => {
            let mut error = GrapheneError::new(summary.code, summary.kind, summary.message);
            for (key, value) in summary.context.iter() {
                error = error.with_context(key, value);
            }
            Err(error)
        }
    }
}
