use graphene::{ErrorCode, ErrorKind, GrapheneError};
use tracing_subscriber::{EnvFilter, fmt};

/// Initializes host-owned structured tracing with safe defaults.
///
/// Levels are `warn` (default), `info`, `debug`, and `trace` for increasing verbosity; the
/// `RUST_LOG` environment variable overrides the numeric default when set. `ansi` is disabled for
/// machine-readable output modes. The subscriber never receives secret values from the engine.
pub fn init_tracing(verbosity: u8, ansi: bool) -> Result<(), GrapheneError> {
    let default_level = match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));
    let subscriber = fmt()
        .with_env_filter(filter)
        .with_ansi(ansi)
        .with_target(false)
        .finish();

    tracing::subscriber::set_global_default(subscriber).map_err(|source| {
        GrapheneError::new(
            ErrorCode::ConfigInvalid,
            ErrorKind::Configuration,
            "tracing subscriber was already initialized",
        )
        .with_source(source)
    })
}
