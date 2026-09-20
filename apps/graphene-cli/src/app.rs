use crate::cli::{Cli, Command};
use crate::commands;
use crate::context::AppContext;
use crate::error::{ErrorBody, exit_code};
use crate::output::{OutputMode, print_error, print_success};
use clap::Parser;
use graphene::{ErrorCode, ErrorKind, GrapheneError};
use graphene_reference_host_support::{HostConfig, KeyringSecretStore, init_tracing};
use std::sync::Arc;

/// Parses arguments, runs the selected command, and exits with the documented coarse code.
pub fn main_entry() {
    let cli = Cli::parse();
    let mode = OutputMode::from_json_flag(cli.json);
    let exit = match run(cli, mode) {
        Ok(()) => 0,
        Err(error) => {
            print_error(mode, &ErrorBody::from_error(&error));
            exit_code(&error)
        }
    };

    std::process::exit(exit);
}

fn run(cli: Cli, mode: OutputMode) -> Result<(), GrapheneError> {
    // Executable hosts own tracing; a duplicate initialization is never fatal.
    let _ = init_tracing(cli.verbose, !mode.is_json());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|source| {
            GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                "failed to start the host runtime",
            )
            .with_source(source)
        })?;

    runtime.block_on(dispatch(cli, mode))
}

async fn dispatch(cli: Cli, mode: OutputMode) -> Result<(), GrapheneError> {
    let config = HostConfig::from_env(cli.data_root)?;
    let engine = config.build(Arc::new(KeyringSecretStore::new())).await?;
    let context = AppContext {
        engine,
        output: mode,
    };

    let rendered = match cli.command {
        Command::Engine(args) => commands::engine::dispatch(&context, args).await?,
        Command::Instance(args) => commands::instance::dispatch(&context, args).await?,
        Command::Install(args) => commands::install::dispatch(&context, args).await?,
        Command::Account(args) => commands::account::dispatch(&context, args).await?,
        Command::Java(args) => commands::java::dispatch(&context, args).await?,
        Command::Content(args) => commands::content::dispatch(&context, args).await?,
        Command::Modpack(args) => commands::modpack::dispatch(&context, args).await?,
        Command::Launch(args) => commands::launch::dispatch(&context, args).await?,
        Command::Diagnose(args) => commands::diagnose::dispatch(&context, args).await?,
    };

    print_success(mode, &rendered);
    Ok(())
}
