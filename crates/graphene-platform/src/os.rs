/// Normalized operating system identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperatingSystem {
    Windows,
    Linux,
    MacOS,
    Other,
}

impl OperatingSystem {
    /// Detects the compile target's operating system.
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "macos") {
            Self::MacOS
        } else {
            Self::Other
        }
    }

    /// Normalizes a Rust target OS name.
    #[must_use]
    pub fn normalize(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "windows" => Self::Windows,
            "linux" => Self::Linux,
            "macos" | "darwin" => Self::MacOS,
            _ => Self::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_known_os_names() {
        assert_eq!(
            OperatingSystem::normalize("Windows"),
            OperatingSystem::Windows
        );
        assert_eq!(OperatingSystem::normalize("darwin"), OperatingSystem::MacOS);
        assert_eq!(OperatingSystem::normalize("linux"), OperatingSystem::Linux);
        assert_eq!(OperatingSystem::normalize("plan9"), OperatingSystem::Other);
    }
}
