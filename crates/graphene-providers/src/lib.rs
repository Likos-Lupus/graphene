//! Official-compatible provider normalization for Vanilla Minecraft metadata.
//!
//! Provider DTOs remain private inside the Mojang adapter. The crate root deliberately exposes
//! only Graphene-facing provider configuration, acquisition ports, and normalized results.

mod java_distribution;
mod loader;
mod microsoft;
mod mojang;

pub use mojang::{
    MetadataArtifactAcquirer, MojangProvider, MojangProviderConfig, ResolvedMinecraftBundle,
};

pub use java_distribution::{AdoptiumProvider, AdoptiumProviderConfig};
pub use microsoft::{MicrosoftAuthConfig, MicrosoftAuthProvider, MicrosoftServiceEndpoints};

pub use loader::{
    FabricProvider, FabricProviderConfig, ForgeProvider, ForgeProviderConfig, LoaderProvider,
    LoaderProviderFuture, LoaderProviderRegistry, NeoForgeProvider, NeoForgeProviderConfig,
};
