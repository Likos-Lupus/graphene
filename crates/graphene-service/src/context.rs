use graphene_auth::{AccountRepository, AuthProvider, SecretStore};
use graphene_core::{AccountId, ManagedRuntimeId, OperationRegistry};
use graphene_java::JavaDistributionProvider;
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
    pub account_repository: Arc<dyn AccountRepository>,
    pub secret_store: Arc<dyn SecretStore>,
    pub auth_provider: Option<Arc<dyn AuthProvider>>,
    pub java_distribution_provider: Arc<dyn JavaDistributionProvider>,
    pub artifact_gates: Mutex<HashMap<PathBuf, Weak<AsyncMutex<()>>>>,
    pub account_gates: Mutex<HashMap<AccountId, Weak<AsyncMutex<()>>>>,
    pub runtime_gates: Mutex<HashMap<ManagedRuntimeId, Weak<AsyncMutex<()>>>>,
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

    pub fn account_gate(&self, id: AccountId) -> Arc<AsyncMutex<()>> {
        let mut gates = self
            .account_gates
            .lock()
            .expect("account gate map poisoned");
        gates.retain(|_, gate| gate.strong_count() > 0);
        if let Some(gate) = gates.get(&id).and_then(Weak::upgrade) {
            return gate;
        }

        let gate = Arc::new(AsyncMutex::new(()));
        gates.insert(id, Arc::downgrade(&gate));
        gate
    }

    pub fn runtime_gate(&self, id: ManagedRuntimeId) -> Arc<AsyncMutex<()>> {
        let mut gates = self
            .runtime_gates
            .lock()
            .expect("runtime gate map poisoned");
        gates.retain(|_, gate| gate.strong_count() > 0);
        if let Some(gate) = gates.get(&id).and_then(Weak::upgrade) {
            return gate;
        }

        let gate = Arc::new(AsyncMutex::new(()));
        gates.insert(id, Arc::downgrade(&gate));
        gate
    }
}
