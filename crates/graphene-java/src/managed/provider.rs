use super::{JavaImageKind, JavaOperatingSystem, ManagedJavaRelease, ManagedJavaRequest};
use crate::JavaArchitecture;
use graphene_core::{OperationController, Result};
use std::{future::Future, pin::Pin};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaDistributionCapabilities {
    pub sha256: bool,
    pub jre: bool,
    pub jdk: bool,
    pub windows: bool,
    pub linux: bool,
    pub macos: bool,
    pub x86: bool,
    pub x86_64: bool,
    pub aarch64: bool,
}

impl JavaDistributionCapabilities {
    #[must_use]
    pub fn supports(&self, request: &ManagedJavaRequest) -> bool {
        self.sha256
            && match request.preferred_image {
                JavaImageKind::Jre => self.jre,
                JavaImageKind::Jdk => self.jdk,
            }
            && match request.os {
                JavaOperatingSystem::Windows => self.windows,
                JavaOperatingSystem::Linux => self.linux,
                JavaOperatingSystem::MacOS => self.macos,
                JavaOperatingSystem::Other => false,
            }
            && match request.architecture {
                JavaArchitecture::X86 => self.x86,
                JavaArchitecture::X86_64 => self.x86_64,
                JavaArchitecture::AArch64 => self.aarch64,
                JavaArchitecture::Other => false,
            }
    }
}

pub type JavaDistributionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ManagedJavaRelease>> + Send + 'a>>;

pub trait JavaDistributionProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;
    fn capabilities(&self) -> JavaDistributionCapabilities;
    fn resolve_release<'a>(
        &'a self,
        request: &'a ManagedJavaRequest,
        operation: &'a OperationController,
    ) -> JavaDistributionFuture<'a>;
}
