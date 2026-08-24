pub mod descriptor;
pub mod fabric;
pub mod forge;
pub mod legacy;
pub mod neoforge;

pub use descriptor::{LocalModDependency, ModMetadataSource, NormalizedModDescriptor};

use graphene_core::{
    CancellationToken, ErrorCode, ErrorKind, GrapheneError, Result,
    archive::{central_entries, extract_entry},
};

pub const MAX_ARCHIVE_ENTRIES_TO_INSPECT: usize = 2048;
pub const MAX_ENTRY_NAME_BYTES: usize = 512;
pub const MAX_METADATA_DECOMPRESSED_BYTES: usize = 1024 * 1024; // 1 MiB

/// Inspects a mod JAR archive bytes and extracts normalized mod metadata without extracting the entire archive.
pub fn inspect_mod_archive(
    archive_bytes: &[u8],
    cancellation: &CancellationToken,
) -> Result<Vec<NormalizedModDescriptor>> {
    let entries = central_entries(
        archive_bytes,
        MAX_ARCHIVE_ENTRIES_TO_INSPECT,
        MAX_ENTRY_NAME_BYTES,
    )
    .map_err(|err| {
        GrapheneError::new(
            ErrorCode::LoaderMetadataInvalid,
            ErrorKind::Minecraft,
            format!("failed to parse archive central directory: {err}"),
        )
    })?;

    // 1. Check for Fabric metadata (fabric.mod.json)
    if let Some(entry) = entries.iter().find(|e| e.name == "fabric.mod.json") {
        let bytes = extract_entry(
            archive_bytes,
            entry,
            MAX_METADATA_DECOMPRESSED_BYTES,
            cancellation,
        )
        .map_err(|err| {
            GrapheneError::new(
                ErrorCode::LoaderMetadataInvalid,
                ErrorKind::Minecraft,
                format!("failed to extract fabric.mod.json: {err}"),
            )
        })?;
        let descriptor = fabric::parse_fabric_mod_json(&bytes)?;
        return Ok(vec![descriptor]);
    }

    // 2. Check for NeoForge metadata (META-INF/neoforge.mods.toml)
    if let Some(entry) = entries
        .iter()
        .find(|e| e.name == "META-INF/neoforge.mods.toml")
    {
        let bytes = extract_entry(
            archive_bytes,
            entry,
            MAX_METADATA_DECOMPRESSED_BYTES,
            cancellation,
        )
        .map_err(|err| {
            GrapheneError::new(
                ErrorCode::LoaderMetadataInvalid,
                ErrorKind::Minecraft,
                format!("failed to extract META-INF/neoforge.mods.toml: {err}"),
            )
        })?;
        return neoforge::parse_neoforge_mods_toml(&bytes);
    }

    // 3. Check for Forge metadata (META-INF/mods.toml)
    if let Some(entry) = entries.iter().find(|e| e.name == "META-INF/mods.toml") {
        let bytes = extract_entry(
            archive_bytes,
            entry,
            MAX_METADATA_DECOMPRESSED_BYTES,
            cancellation,
        )
        .map_err(|err| {
            GrapheneError::new(
                ErrorCode::LoaderMetadataInvalid,
                ErrorKind::Minecraft,
                format!("failed to extract META-INF/mods.toml: {err}"),
            )
        })?;
        return forge::parse_forge_mods_toml(&bytes, ModMetadataSource::Forge);
    }

    // 4. Check for legacy mcmod.info
    if let Some(entry) = entries.iter().find(|e| e.name == "mcmod.info") {
        let bytes = extract_entry(
            archive_bytes,
            entry,
            MAX_METADATA_DECOMPRESSED_BYTES,
            cancellation,
        )
        .map_err(|err| {
            GrapheneError::new(
                ErrorCode::LoaderMetadataInvalid,
                ErrorKind::Minecraft,
                format!("failed to extract mcmod.info: {err}"),
            )
        })?;
        return legacy::parse_mcmod_info(&bytes);
    }

    // Unknown valid archive with no known mod metadata
    Ok(Vec::new())
}
