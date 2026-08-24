pub mod config;
pub(crate) mod dto;
pub(crate) mod normalize;
pub mod provider;
#[cfg(test)]
mod tests;

pub use config::CurseForgeProviderConfig;
pub use provider::CurseForgeContentProvider;
