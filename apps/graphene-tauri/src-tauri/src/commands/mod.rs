pub mod accounts;
pub mod content;
pub mod diagnostics;
pub mod engine;
pub mod install;
pub mod instances;
pub mod java;
pub mod launch;
pub mod modpacks;
pub mod operations;

use crate::error::HostError;
use graphene::LoaderKind;

pub(crate) fn parse_loader(value: &str) -> Result<LoaderKind, HostError> {
    match value {
        "fabric" => Ok(LoaderKind::Fabric),
        "forge" => Ok(LoaderKind::Forge),
        "neoforge" => Ok(LoaderKind::NeoForge),
        _ => Err(HostError::invalid("loader", value)),
    }
}
