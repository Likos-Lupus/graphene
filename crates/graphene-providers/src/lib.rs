//! Official-compatible provider normalization for Vanilla Minecraft metadata.
//!
//! Provider DTOs remain private inside the Mojang adapter. The crate root deliberately exposes
//! only Graphene-facing provider configuration, acquisition ports, and normalized results.

mod mojang;

pub use mojang::{
    MetadataArtifactAcquirer, MojangProvider, MojangProviderConfig, ResolvedMinecraftBundle,
};
