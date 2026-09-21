use crate::output::OutputMode;
use graphene::{CancellationToken, Graphene};

/// Host-owned runtime state for one CLI invocation. No process-global engine singleton.
pub struct AppContext {
    pub engine: Graphene,
    pub output: OutputMode,
    /// Process interrupt token installed at startup so Ctrl+C is honored even during engine
    /// construction.
    pub interrupt: CancellationToken,
}
