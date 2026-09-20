use graphene::{GameEvent, OperationEvent, OperationEventKind, OperationId};
use graphene_reference_host_support::{
    HostOperationRegistry, HostOperationTerminal, HostRunRegistry, RunEventEnvelope,
};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// Bounded typed channels used for all host-to-webview forwarding.
pub const OPERATION_EVENT: &str = "graphene://operation";
pub const OPERATION_TERMINAL_EVENT: &str = "graphene://operation-terminal";
pub const GAME_EVENT: &str = "graphene://game";
pub const AUTH_RESULT_EVENT: &str = "graphene://auth-result";

#[derive(Debug, Clone, Serialize)]
pub struct OperationEventWire {
    pub operation_id: String,
    pub sequence: u64,
    pub kind: OperationEventKind,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationTerminalWire {
    pub operation_id: String,
    pub terminal: Option<HostOperationTerminal>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GameEventWire {
    pub run_id: String,
    pub kind: String,
    pub pid: Option<u32>,
    pub text: Option<String>,
    pub success: Option<bool>,
    pub exit_code: Option<i32>,
}

/// Forwards a tracked operation's bounded events until its terminal event is retained.
pub fn forward_operation(app: AppHandle, registry: Arc<HostOperationRegistry>, id: OperationId) {
    let Some(mut receiver) = registry.subscribe(id) else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        while let Ok(event) = receiver.recv().await {
            let OperationEvent {
                operation_id,
                sequence,
                kind,
                ..
            } = event;
            let wire = OperationEventWire {
                operation_id: operation_id.to_string(),
                sequence,
                kind,
            };
            let _ = app.emit(OPERATION_EVENT, wire);
        }

        let terminal = registry.terminal(id);
        let _ = app.emit(
            OPERATION_TERMINAL_EVENT,
            OperationTerminalWire {
                operation_id: id.to_string(),
                terminal,
            },
        );
    });
}

/// Forwards bounded game events for one run id from the shared run channel.
pub fn forward_run(app: AppHandle, registry: Arc<HostRunRegistry>, run_id: String) {
    let mut receiver = registry.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(envelope) => {
                    if envelope.run_id == run_id {
                        let _ = app.emit(GAME_EVENT, game_event_wire(envelope));
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

fn game_event_wire(envelope: RunEventEnvelope) -> GameEventWire {
    let run_id = envelope.run_id;
    match envelope.event {
        GameEvent::Started { pid } => GameEventWire {
            run_id,
            kind: "started".to_owned(),
            pid: Some(pid),
            text: None,
            success: None,
            exit_code: None,
        },

        GameEvent::Stdout { text, .. } => GameEventWire {
            run_id,
            kind: "stdout".to_owned(),
            pid: None,
            text: Some(text),
            success: None,
            exit_code: None,
        },

        GameEvent::Stderr { text, .. } => GameEventWire {
            run_id,
            kind: "stderr".to_owned(),
            pid: None,
            text: Some(text),
            success: None,
            exit_code: None,
        },

        GameEvent::Exited { exit } => GameEventWire {
            run_id,
            kind: "exited".to_owned(),
            pid: None,
            text: None,
            success: Some(exit.success),
            exit_code: exit.code,
        },

        GameEvent::Failed { code } => GameEventWire {
            run_id,
            kind: "failed".to_owned(),
            pid: None,
            text: Some(code.as_str().to_owned()),
            success: None,
            exit_code: None,
        },

        _ => GameEventWire {
            run_id,
            kind: "unknown".to_owned(),
            pid: None,
            text: None,
            success: None,
            exit_code: None,
        },
    }
}
