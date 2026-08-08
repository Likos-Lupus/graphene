use graphene_core::OperationRegistry;
use graphene_network::NetworkClient;
use graphene_platform::Platform;
use graphene_providers::MojangProviderConfig;
use graphene_storage::DataRoot;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::Mutex as AsyncMutex;

pub(crate) struct ServiceContext {
    pub platform: Platform,
    pub storage: DataRoot,
    pub network: NetworkClient,
    pub operations: OperationRegistry,
    pub provider_config: MojangProviderConfig,
    pub artifact_gates: Mutex<HashMap<PathBuf, Weak<AsyncMutex<()>>>>,
}

impl ServiceContext {
    pub fn artifact_gate(&self, path: &Path) -> Arc<AsyncMutex<()>> {
        let mut gates = self
            .artifact_gates
            .lock()
            .expect("artifact gate map poisoned");
        gates.retain(|_, gate| gate.strong_count() > 0);
        if let Some(gate) = gates.get(path).and_then(Weak::upgrade) {
            return gate;
        }

        let gate = Arc::new(AsyncMutex::new(()));
        gates.insert(path.to_path_buf(), Arc::downgrade(&gate));
        gate
    }
}
