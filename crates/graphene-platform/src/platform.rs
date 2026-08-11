use crate::{Architecture, OperatingSystem};

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
