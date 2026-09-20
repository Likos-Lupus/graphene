use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Reference Graphene CLI.
#[derive(Debug, Parser)]
#[command(
    name = "graphene-cli",
    version,
    about = "Reference Graphene launcher CLI",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Explicit engine data root. Falls back to `GRAPHENE_DATA_ROOT` or a platform default.
    #[arg(long, global = true, value_name = "PATH")]
    pub data_root: Option<PathBuf>,

    /// Emit stable machine-readable JSON without ANSI control sequences.
    #[arg(long, global = true)]
    pub json: bool,

    /// Increase host verbosity (`-v` info, `-vv` debug, `-vvv` trace).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Engine status and deterministic reference operations.
    Engine(EngineArgs),
    /// Instance inventory, verification, repair, and lifecycle.
    Instance(InstanceArgs),
    /// Vanilla and loader installation.
    Install(InstallArgs),
    /// Account inventory and authentication.
    Account(AccountArgs),
    /// Java discovery and managed runtimes.
    Java(JavaArgs),
    /// Offline and provider-backed content management.
    Content(ContentArgs),
    /// Modpack inspection, import, and export.
    Modpack(ModpackArgs),
    /// Launch planning and process lifecycle.
    Launch(LaunchArgs),
    /// Structured, read-only diagnostics.
    Diagnose(DiagnoseArgs),
}

#[derive(Debug, Args)]
pub struct EngineArgs {
    #[command(subcommand)]
    pub command: EngineCommand,
}

#[derive(Debug, Subcommand)]
pub enum EngineCommand {
    /// Print data root, platform, and engine identity.
    Info,
    /// Run a deterministic synthetic operation and render its bounded events.
    Synthetic {
        #[arg(long, default_value_t = 5)]
        steps: u64,
        #[arg(long, default_value_t = 20)]
        delay_ms: u64,
    },
}

#[derive(Debug, Args)]
pub struct InstanceArgs {
    #[command(subcommand)]
    pub command: InstanceCommand,
}

#[derive(Debug, Subcommand)]
pub enum InstanceCommand {
    /// List committed instances.
    List,
    /// Print one committed instance.
    Get { instance_id: String },
    /// Verify managed state without mutation.
    Verify {
        instance_id: String,
        #[arg(long)]
        full: bool,
    },
    /// Plan (and optionally execute) deterministic repair.
    Repair {
        instance_id: String,
        #[arg(long)]
        execute: bool,
        #[arg(long)]
        full: bool,
    },
    /// Clone an instance under a new display name.
    Clone {
        instance_id: String,
        #[arg(long)]
        name: String,
    },
    /// Delete an instance.
    Delete { instance_id: String },
    /// Read instance configuration.
    Config(InstanceConfigArgs),
}

#[derive(Debug, Args)]
pub struct InstanceConfigArgs {
    #[command(subcommand)]
    pub command: InstanceConfigCommand,
}

#[derive(Debug, Subcommand)]
pub enum InstanceConfigCommand {
    /// Print global instance defaults.
    Global,
    /// Print the effective configuration for an instance.
    Effective { instance_id: String },
}

#[derive(Debug, Args)]
pub struct InstallArgs {
    #[command(subcommand)]
    pub command: InstallCommand,
}

#[derive(Debug, Subcommand)]
pub enum InstallCommand {
    /// Plan and execute a Vanilla install.
    Vanilla {
        #[arg(long)]
        minecraft: String,
        #[arg(long)]
        name: String,
    },
    /// Plan and execute a loader install.
    Loader {
        #[arg(long)]
        minecraft: String,
        #[arg(long, value_enum)]
        loader: LoaderArg,
        #[arg(long)]
        loader_version: Option<String>,
        #[arg(long)]
        name: String,
    },
}

#[derive(Debug, Args)]
pub struct AccountArgs {
    #[command(subcommand)]
    pub command: AccountCommand,
}

#[derive(Debug, Subcommand)]
pub enum AccountCommand {
    /// List stored accounts.
    List,
    /// Add an offline account.
    OfflineAdd {
        #[arg(long)]
        name: String,
    },
    /// Begin a Microsoft device-code login.
    MicrosoftLogin,
    /// Reauthenticate an existing Microsoft account.
    Reauth { account_id: String },
    /// Remove an account and its credential.
    Remove { account_id: String },
}

#[derive(Debug, Args)]
pub struct JavaArgs {
    #[command(subcommand)]
    pub command: JavaCommand,
}

#[derive(Debug, Subcommand)]
pub enum JavaCommand {
    /// List installed managed runtimes.
    List,
    /// Select/ensure Java for an instance.
    Ensure { instance_id: String },
    /// Install a managed runtime for a major version.
    Install {
        #[arg(long)]
        major: u32,
    },
}

#[derive(Debug, Args)]
pub struct ContentArgs {
    #[command(subcommand)]
    pub command: ContentCommand,
}

#[derive(Debug, Subcommand)]
pub enum ContentCommand {
    /// Scan the local mods directory strictly offline.
    Scan {
        instance_id: String,
        #[arg(long)]
        hashes: bool,
    },
    /// Search a remote provider catalog.
    Search {
        #[arg(long, value_enum, default_value_t = ProviderArg::Modrinth)]
        provider: ProviderArg,
        #[arg(long)]
        query: String,
        #[arg(long)]
        minecraft: Option<String>,
        #[arg(long, value_enum)]
        loader: Option<LoaderArg>,
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// Recognize local files against remote providers.
    Recognize { instance_id: String },
    /// Plan (and optionally execute) installing an exact remote version.
    Install {
        instance_id: String,
        #[arg(long, value_enum, default_value_t = ProviderArg::Modrinth)]
        provider: ProviderArg,
        #[arg(long)]
        project: String,
        #[arg(long)]
        version: String,
        #[arg(long)]
        execute: bool,
    },
    /// Plan (and optionally execute) available managed updates.
    Update {
        instance_id: String,
        #[arg(long)]
        execute: bool,
    },
}

#[derive(Debug, Args)]
pub struct ModpackArgs {
    #[command(subcommand)]
    pub command: ModpackCommand,
}

#[derive(Debug, Subcommand)]
pub enum ModpackCommand {
    /// Inspect a local modpack file or HTTPS URL.
    Inspect { source: String },
    /// Plan (and optionally execute) a modpack import.
    Import {
        source: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        execute: bool,
    },
    /// Plan and execute a conservative Graphene pack export.
    Export {
        instance_id: String,
        #[arg(long)]
        name: String,
        #[arg(long, value_name = "PATH")]
        out: PathBuf,
    },
}

#[derive(Debug, Args)]
pub struct LaunchArgs {
    #[command(subcommand)]
    pub command: LaunchCommand,
}

#[derive(Debug, Subcommand)]
pub enum LaunchCommand {
    /// Reconstruct a redacted launch plan.
    Plan {
        instance_id: String,
        #[arg(long)]
        account: Option<String>,
    },
    /// Execute a launch plan in the foreground with Ctrl+C kill semantics.
    Run {
        instance_id: String,
        #[arg(long)]
        account: Option<String>,
    },
    /// Terminate a previously launched game process by pid.
    Kill {
        #[arg(long)]
        pid: u32,
    },
}

#[derive(Debug, Args)]
pub struct DiagnoseArgs {
    /// Instance to diagnose.
    pub instance_id: String,
    #[arg(long, value_enum, default_value_t = DiagnosticModeArg::Preflight)]
    pub mode: DiagnosticModeArg,
    /// Optional observed process exit code.
    #[arg(long)]
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LoaderArg {
    Fabric,
    Forge,
    NeoForge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ProviderArg {
    Modrinth,
    Curseforge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DiagnosticModeArg {
    Preflight,
    Crash,
    Full,
}
