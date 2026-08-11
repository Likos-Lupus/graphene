//! OS-facing primitives for Graphene.
//!
//! This crate normalizes platform identity and owns low-level managed-path, directory, and safe
//! replacement behavior. It intentionally contains no launcher domain policy.

mod arch;
mod filesystem;
mod os;
mod paths;
mod platform;
mod process;

pub use arch::Architecture;
pub use filesystem::{ensure_directory, ensure_managed_directory, replace_file_safely};
pub use os::OperatingSystem;
pub use paths::{ManagedRelativePath, normalize_root, publish_directory_create_only};
pub use platform::Platform;
pub use process::{
    CapturedProcess, PlatformProcess, ProcessExit, ProcessOutput, ProcessSpec, classpath_separator,
    executable_exists, os_arg_is_empty, run_process_bounded,
};
