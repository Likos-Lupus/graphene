use graphene_core::ErrorCode;
use tokio::sync::mpsc;

/// Observable game lifecycle event. Output retains raw bounded chunks and deterministic lossy text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GameEvent {
    Started { pid: u32 },
    Stdout { bytes: Vec<u8>, text: String },
    Stderr { bytes: Vec<u8>, text: String },
    Exited { exit: GameExit },
    Failed { code: ErrorCode },
}

pub struct GameEventStream {
    pub(super) receiver: mpsc::Receiver<GameEvent>,
}

impl GameEventStream {
    pub async fn next(&mut self) -> Option<GameEvent> {
        self.receiver.recv().await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameExit {
    pub success: bool,
    pub code: Option<i32>,
    pub killed: bool,
}
