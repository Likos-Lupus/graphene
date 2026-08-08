use graphene_platform::Architecture;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaRequirement {
    pub major_version: u32,
    pub component_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub enum JavaVendor {
    Adoptium,
    Oracle,
    Microsoft,
    Azul,
    Amazon,
    GraalVm,
    Other(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub enum JavaArchitecture {
    X86,
    X86_64,
    AArch64,
    Other,
}

impl JavaArchitecture {
    #[must_use]
    pub fn normalize(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "x86" | "i386" | "i486" | "i586" | "i686" => Self::X86,
            "x86_64" | "amd64" | "x64" => Self::X86_64,
            "aarch64" | "arm64" => Self::AArch64,
            _ => Self::Other,
        }
    }

    #[must_use]
    pub const fn current() -> Self {
        match Architecture::current() {
            Architecture::X86 => Self::X86,
            Architecture::X86_64 => Self::X86_64,
            Architecture::AArch64 => Self::AArch64,
            Architecture::Other => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum JavaCandidateSource {
    Explicit,
    JavaHome,
    Path,
    CommonRoot,
}

impl JavaCandidateSource {
    pub(crate) const fn priority(self) -> u8 {
        match self {
            Self::Explicit => 0,
            Self::JavaHome => 1,
            Self::Path => 2,
            Self::CommonRoot => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaCandidate {
    pub executable: PathBuf,
    pub source: JavaCandidateSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaRuntime {
    pub executable: PathBuf,
    pub version: String,
    pub major_version: u32,
    pub vendor: JavaVendor,
    pub architecture: JavaArchitecture,
    pub java_home: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_architecture() {
        assert_eq!(
            JavaArchitecture::normalize("AMD64"),
            JavaArchitecture::X86_64
        );
        assert_eq!(
            JavaArchitecture::normalize("arm64"),
            JavaArchitecture::AArch64
        );
    }
}
