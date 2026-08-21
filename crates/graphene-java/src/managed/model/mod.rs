use crate::{JavaArchitecture, JavaRequirement, JavaRuntime, JavaVendor};
use graphene_core::{
    Artifact, ErrorCode, ErrorKind, GrapheneError, ManagedRuntimeId, Result, Sha256Digest,
};
use graphene_platform::OperatingSystem;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MANAGED_RUNTIME_SCHEMA_VERSION: u32 = 1;
pub const MANAGED_JAVA_PLAN_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum JavaOperatingSystem {
    Windows,
    Linux,
    MacOS,
    Other,
}

impl JavaOperatingSystem {
    #[must_use]
    pub const fn current() -> Self {
        match OperatingSystem::current() {
            OperatingSystem::Windows => Self::Windows,
            OperatingSystem::Linux => Self::Linux,
            OperatingSystem::MacOS => Self::MacOS,
            OperatingSystem::Other => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum JavaImageKind {
    Jre,
    Jdk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ManagedArchiveFormat {
    Zip,
    TarGz,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedJavaRequest {
    pub major_version: u32,
    pub os: JavaOperatingSystem,
    pub architecture: JavaArchitecture,
    pub preferred_image: JavaImageKind,
}

impl ManagedJavaRequest {
    pub fn for_current_platform(requirement: &JavaRequirement) -> Result<Self> {
        if requirement.major_version == 0 {
            return Err(GrapheneError::new(
                ErrorCode::JavaIncompatible,
                ErrorKind::Java,
                "managed Java request has invalid major version",
            ));
        }

        let request = Self {
            major_version: requirement.major_version,
            os: JavaOperatingSystem::current(),
            architecture: JavaArchitecture::current(),
            preferred_image: JavaImageKind::Jre,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<()> {
        if self.major_version == 0
            || self.os == JavaOperatingSystem::Other
            || self.architecture == JavaArchitecture::Other
        {
            return Err(GrapheneError::new(
                ErrorCode::PlatformUnsupported,
                ErrorKind::Java,
                "managed Java request uses an unsupported platform",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedJavaRelease {
    pub runtime_id: ManagedRuntimeId,
    pub provider: String,
    pub distribution: String,
    pub release_name: String,
    pub version: String,
    pub major_version: u32,
    pub vendor: JavaVendor,
    pub os: JavaOperatingSystem,
    pub architecture: JavaArchitecture,
    pub image: JavaImageKind,
    pub archive_format: ManagedArchiveFormat,
    pub archive_sha256: Sha256Digest,
    pub artifact: Artifact,
}

impl ManagedJavaRelease {
    pub fn validate(&self) -> Result<()> {
        validate_release_strings(
            &self.provider,
            &self.distribution,
            &self.release_name,
            &self.version,
            &self.vendor,
        )?;

        if self.major_version == 0
            || self.os == JavaOperatingSystem::Other
            || self.architecture == JavaArchitecture::Other
        {
            return Err(release_unavailable(
                "managed Java release targets an unsupported platform",
            ));
        }

        if self.artifact.integrity.sha256() != Some(self.archive_sha256)
            || self.artifact.sources.is_empty()
        {
            return Err(release_unavailable(
                "managed Java release lacks required SHA-256 integrity metadata",
            ));
        }

        if self
            .artifact
            .sources
            .iter()
            .any(|source| source.url().is_empty() || source.url().len() > 2048)
        {
            return Err(release_unavailable(
                "managed Java release contains an invalid artifact source",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedJavaInstallPlan {
    pub plan_version: u32,
    pub request: ManagedJavaRequest,
    pub release: ManagedJavaRelease,
}

impl ManagedJavaInstallPlan {
    pub fn new(request: ManagedJavaRequest, release: ManagedJavaRelease) -> Result<Self> {
        let plan = Self {
            plan_version: MANAGED_JAVA_PLAN_VERSION,
            request,
            release,
        };
        plan.validate()?;

        Ok(plan)
    }

    pub fn validate(&self) -> Result<()> {
        self.request.validate()?;
        self.release.validate()?;
        if self.plan_version != MANAGED_JAVA_PLAN_VERSION
            || self.release.major_version != self.request.major_version
            || self.release.os != self.request.os
            || self.release.architecture != self.request.architecture
            || !image_satisfies(self.request.preferred_image, self.release.image)
        {
            return Err(release_unavailable(
                "managed Java install plan is incompatible with its request",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedJavaRuntime {
    pub schema_version: u32,
    pub id: ManagedRuntimeId,
    pub provider: String,
    pub distribution: String,
    pub release_name: String,
    pub version: String,
    pub major_version: u32,
    pub vendor: JavaVendor,
    pub os: JavaOperatingSystem,
    pub architecture: JavaArchitecture,
    pub image: JavaImageKind,
    pub archive_sha256: Sha256Digest,
    pub executable_relative_path: String,
}

impl ManagedJavaRuntime {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != MANAGED_RUNTIME_SCHEMA_VERSION || self.major_version == 0 {
            return Err(corrupt("managed runtime descriptor is invalid"));
        }

        validate_runtime_strings(
            &self.provider,
            &self.distribution,
            &self.release_name,
            &self.version,
            &self.vendor,
        )?;
        if self.os == JavaOperatingSystem::Other || self.architecture == JavaArchitecture::Other {
            return Err(corrupt(
                "managed runtime descriptor has an unsupported platform",
            ));
        }

        if self.executable_relative_path.is_empty() || self.executable_relative_path.len() > 1024 {
            return Err(corrupt("managed runtime executable path is invalid"));
        }

        let path = Path::new(&self.executable_relative_path);
        if path.is_absolute()
            || self.executable_relative_path.contains('\\')
            || self
                .executable_relative_path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(corrupt("managed runtime executable path is unsafe"));
        }

        Ok(())
    }

    pub fn to_java_runtime(&self, committed_runtime_dir: &Path) -> Result<JavaRuntime> {
        self.validate()?;
        let executable = committed_runtime_dir.join(&self.executable_relative_path);
        let java_home = executable
            .parent()
            .and_then(Path::parent)
            .map(PathBuf::from);
        Ok(JavaRuntime {
            executable,
            version: self.version.clone(),
            major_version: self.major_version,
            vendor: self.vendor.clone(),
            architecture: self.architecture,
            java_home,
        })
    }
}

const fn image_satisfies(requested: JavaImageKind, provided: JavaImageKind) -> bool {
    matches!(
        (requested, provided),
        (JavaImageKind::Jre, JavaImageKind::Jre | JavaImageKind::Jdk)
            | (JavaImageKind::Jdk, JavaImageKind::Jdk)
    )
}

fn validate_release_strings(
    provider: &str,
    distribution: &str,
    release_name: &str,
    version: &str,
    vendor: &JavaVendor,
) -> Result<()> {
    if !bounded_metadata(provider, 64)
        || !bounded_metadata(distribution, 64)
        || !bounded_metadata(release_name, 160)
        || !bounded_metadata(version, 64)
        || !vendor_is_bounded(vendor)
    {
        return Err(release_unavailable(
            "managed Java release metadata is outside Graphene bounds",
        ));
    }

    Ok(())
}

fn validate_runtime_strings(
    provider: &str,
    distribution: &str,
    release_name: &str,
    version: &str,
    vendor: &JavaVendor,
) -> Result<()> {
    if !bounded_metadata(provider, 64)
        || !bounded_metadata(distribution, 64)
        || !bounded_metadata(release_name, 160)
        || !bounded_metadata(version, 64)
        || !vendor_is_bounded(vendor)
    {
        return Err(corrupt(
            "managed runtime metadata is outside Graphene bounds",
        ));
    }

    Ok(())
}

fn bounded_metadata(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

fn vendor_is_bounded(vendor: &JavaVendor) -> bool {
    match vendor {
        JavaVendor::Other(value) => bounded_metadata(value, 64),
        _ => true,
    }
}

fn release_unavailable(message: &'static str) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::JavaManagedReleaseUnavailable,
        ErrorKind::Java,
        message,
    )
}

fn corrupt(message: &'static str) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::JavaManagedRuntimeCorrupt,
        ErrorKind::Java,
        message,
    )
}

#[cfg(test)]
mod tests;
