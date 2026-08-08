//! OS-facing primitives for Graphene.
//!
//! This crate normalizes platform identity and owns low-level managed-path, directory, and safe
//! replacement behavior. It intentionally contains no launcher domain policy.

mod arch;
mod filesystem;
mod os;
mod paths;
mod process;

pub use arch::Architecture;
pub use filesystem::{ensure_directory, ensure_managed_directory, replace_file_safely};
pub use os::OperatingSystem;
pub use paths::{ManagedRelativePath, normalize_root, publish_directory_create_only};
pub use process::{
    CapturedProcess, PlatformProcess, ProcessExit, ProcessOutput, ProcessSpec, classpath_separator,
    executable_exists, os_arg_is_empty, run_process_bounded,
};

/// Normalized platform information captured when an engine is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Platform {
    pub os: OperatingSystem,
    pub architecture: Architecture,
}

impl Platform {
    /// Detects the current target platform without mutating process-global state.
    #[must_use]
    pub const fn current() -> Self {
        Self {
            os: OperatingSystem::current(),
            architecture: Architecture::current(),
        }
    }
}
