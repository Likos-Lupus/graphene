use crate::local::metadata::{
    descriptor::{ModMetadataSource, NormalizedModDescriptor},
    forge::parse_forge_mods_toml,
};
use graphene_core::Result;

pub fn parse_neoforge_mods_toml(bytes: &[u8]) -> Result<Vec<NormalizedModDescriptor>> {
    parse_forge_mods_toml(bytes, ModMetadataSource::NeoForge)
}
