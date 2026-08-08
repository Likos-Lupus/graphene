use super::event::GameEvent;
use graphene_platform::ProcessOutput;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::mpsc;

pub const MAX_PROCESS_CHUNK_BYTES: usize = 4096;

#[derive(Clone, Copy)]
pub(super) enum OutputKind {
    Stdout,
    Stderr,
}

pub(super) async fn drain_output(
    mut output: ProcessOutput,
    kind: OutputKind,
    sender: mpsc::Sender<GameEvent>,
    dropped: Arc<AtomicU64>,
) -> std::io::Result<()> {
    let mut buffer = [0u8; MAX_PROCESS_CHUNK_BYTES];
    loop {
        let count = output.read(&mut buffer).await?;
        if count == 0 {
            return Ok(());
        }

        let bytes = buffer[..count].to_vec();
        let text = String::from_utf8_lossy(&bytes).into_owned();
        // Reserve two slots for terminal lifecycle events. Output is intentionally lossy at the
        // host boundary, never at the OS pipe boundary.
        if sender.capacity() <= 2 {
            dropped.fetch_add(1, Ordering::Relaxed);
            continue;
        }

        let event = match kind {
            OutputKind::Stdout => GameEvent::Stdout { bytes, text },
            OutputKind::Stderr => GameEvent::Stderr { bytes, text },
        };
        if sender.try_send(event).is_err() {
            dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}
