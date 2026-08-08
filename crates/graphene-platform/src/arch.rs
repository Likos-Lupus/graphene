/// Normalized CPU architecture identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Architecture {
    X86,
    X86_64,
    AArch64,
    Other,
}

impl Architecture {
    /// Detects the compile target architecture.
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(target_arch = "x86") {
            Self::X86
        } else if cfg!(target_arch = "x86_64") {
            Self::X86_64
        } else if cfg!(target_arch = "aarch64") {
            Self::AArch64
        } else {
            Self::Other
        }
    }

    /// Normalizes common architecture spellings.
    #[must_use]
    pub fn normalize(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "x86" | "i386" | "i686" => Self::X86,
            "x86_64" | "amd64" => Self::X86_64,
            "aarch64" | "arm64" => Self::AArch64,
            _ => Self::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_known_architectures() {
        assert_eq!(Architecture::normalize("AMD64"), Architecture::X86_64);
        assert_eq!(Architecture::normalize("arm64"), Architecture::AArch64);
        assert_eq!(Architecture::normalize("riscv64"), Architecture::Other);
    }
}
