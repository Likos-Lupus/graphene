//! Reference-host-only support shared by the Graphene CLI and Tauri reference host.
//!
//! This crate is an outward consumer of the root `graphene` facade. It owns host configuration
//! resolution, a production OS credential-vault adapter, executable tracing initialization, and
//! bounded operation/run bridging registries. It contains no Minecraft, provider, install, content,
//! modpack, or diagnostic business logic; those remain in the engine.

mod config;
mod keyring_store;
mod logging;
mod operation;
mod run;

pub use config::{HostConfig, resolve_data_root};
pub use keyring_store::KeyringSecretStore;
pub use logging::init_tracing;
pub use operation::{
    HostOperationRegistry, HostOperationTerminal, OperationSummary, unknown_operation,
};
pub use run::{HostRun, HostRunRegistry, RunEventEnvelope, RunSummary};
