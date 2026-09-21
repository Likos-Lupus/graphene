use graphene::Graphene;
use graphene_reference_host_support::{HostOperationRegistry, HostRunRegistry};
use std::{path::Path, sync::Arc, time::Duration};

const OPERATION_RETENTION: Duration = Duration::from_secs(30);
const RUN_RETENTION: Duration = Duration::from_secs(5);
const EVENT_CAPACITY: usize = 256;

/// Host-owned Tauri application state.
///
/// Holds only host composition and registries: the engine facade, the host operation registry, the
/// running-game registry, and the resolved data root. No engine business logic lives here and the
/// engine is never a process-global singleton.
pub struct TauriAppState {
    engine: Graphene,
    operations: Arc<HostOperationRegistry>,
    runs: Arc<HostRunRegistry>,
    data_root: std::path::PathBuf,
}

impl TauriAppState {
    #[must_use]
    pub fn new(engine: Graphene) -> Self {
        let data_root = engine.data_root().to_path_buf();
        Self {
            engine,
            operations: Arc::new(HostOperationRegistry::new(
                EVENT_CAPACITY,
                OPERATION_RETENTION,
            )),
            runs: Arc::new(HostRunRegistry::new(EVENT_CAPACITY, RUN_RETENTION)),
            data_root,
        }
    }

    #[must_use]
    pub fn engine(&self) -> &Graphene {
        &self.engine
    }

    #[must_use]
    pub fn operations(&self) -> &Arc<HostOperationRegistry> {
        &self.operations
    }

    #[must_use]
    pub fn runs(&self) -> &Arc<HostRunRegistry> {
        &self.runs
    }

    #[must_use]
    pub fn data_root(&self) -> &Path {
        &self.data_root
    }
}
