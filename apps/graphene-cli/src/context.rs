use crate::output::OutputMode;
use graphene::Graphene;

/// Host-owned runtime state for one CLI invocation. No process-global engine singleton.
pub struct AppContext {
    pub engine: Graphene,
    pub output: OutputMode,
}
