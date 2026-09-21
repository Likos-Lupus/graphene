use crate::commands;
use crate::state::TauriAppState;
use graphene_reference_host_support::{HostConfig, KeyringSecretStore};
use std::sync::Arc;
use tauri::{Manager, RunEvent};

/// Builds the shared engine from explicit host configuration and runs the reference host.
pub fn run() {
    let engine = tauri::async_runtime::block_on(async {
        let config = HostConfig::from_env(None).expect("host configuration resolves");
        config
            .build(Arc::new(KeyringSecretStore::new()))
            .await
            .expect("engine builds from host configuration")
    });
    let state = TauriAppState::new(engine);

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::engine::engine_info,
            commands::engine::engine_start_synthetic,
            commands::operations::operations_list,
            commands::operations::operations_cancel,
            commands::operations::operations_snapshot,
            commands::instances::instances_list,
            commands::instances::instance_get,
            commands::instances::instance_verify,
            commands::instances::instance_plan_repair,
            commands::instances::instance_repair,
            commands::accounts::accounts_list,
            commands::accounts::account_offline_add,
            commands::accounts::account_remove,
            commands::accounts::account_begin_microsoft_login,
            commands::java::java_list,
            commands::java::java_ensure,
            commands::content::content_scan,
            commands::content::content_search,
            commands::install::install_vanilla,
            commands::install::install_loader,
            commands::modpacks::modpack_inspect,
            commands::modpacks::modpack_import,
            commands::modpacks::modpack_export,
            commands::launch::launch_plan,
            commands::launch::launch_run,
            commands::launch::launch_kill,
            commands::launch::launch_kill_pid,
            commands::launch::runs_list,
            commands::diagnostics::diagnostics_analyze,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build the Graphene reference host")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                let state = app.state::<TauriAppState>();
                // Explicit shutdown: terminate tracked games so no instance lease is dropped while a
                // child process is still alive.
                tauri::async_runtime::block_on(state.runs().shutdown());
            }
        });
}
