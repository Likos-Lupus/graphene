use super::dto::{GrapheneManagedFileDto, GraphenePackDto, GrapheneSeedFileDto};
use crate::archive::PackArchiveIndex;
use crate::archive::limits::{MAX_MANAGED_FILES, MAX_SEED_ENTRIES};
use crate::error::PackError;
use crate::model::file::{
    ContentHint, EmbeddedPackFile, FileSelection, ManagedSource, NormalizedPackFile,
    ProviderFileRef,
};
use crate::model::format::PackFormat;
use crate::model::metadata::PackMetadata;
use crate::model::modpack::{NormalizedModpack, PackDiagnostic, PackDiagnosticCode};
use crate::model::runtime::{PackLoaderRequirement, PackRuntimeRequirement};
use crate::model::seed::{NormalizedSeedEntry, SeedLayer};
use graphene_core::{ArtifactIntegrity, CancellationToken, ErrorCode, Sha256Digest};
use std::collections::BTreeSet;
use std::io::{Read, Seek, sink};

/// Manifest file name at the effective archive root.
pub(super) const MANIFEST_NAME: &str = "graphene.pack.json";
const OBJECTS_PREFIX: &str = "objects/sha256/";
const SEED_PREFIX: &str = "seed/";
const MAX_PROVIDER_ID_CHARS: usize = 32;
const MAX_SOURCES_PER_FILE: usize = 8;

/// Normalizes a detected Graphene pack v1 archive into the Graphene-owned pack model.
///
/// The format is strict by contract: unknown manifest fields fail parsing, every managed file
/// declares exactly one acquisition strategy (embedded object, provider provenance, or safe
/// sources), embedded object paths are derived from the payload SHA-256 rather than an untrusted
/// filename, and unlisted regular files in reserved payload areas fail import.
pub fn normalize<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    root_prefix: &str,
) -> Result<NormalizedModpack, PackError> {
    let manifest_name = format!("{root_prefix}{MANIFEST_NAME}");
    let bytes = index.read_manifest(&manifest_name)?;
    let dto: GraphenePackDto = serde_json::from_slice(&bytes)
        .map_err(|_| PackError::manifest("graphene.pack.json does not match schema v1"))?;

    if dto.schema_version != 1 {
        return Err(PackError::manifest(format!(
            "unsupported Graphene pack schema version {}",
            dto.schema_version
        )));
    }
    if dto.managed_files.len() > MAX_MANAGED_FILES || dto.seed_files.len() > MAX_SEED_ENTRIES {
        return Err(PackError::new(
            ErrorCode::PackSourceTooLarge,
            "declared pack file count exceeds its bound",
        ));
    }

    let metadata = read_metadata(&dto.pack)?;
    let runtime = read_runtime(&dto.runtime)?;

    let cancellation = CancellationToken::new();
    let mut managed_files = Vec::with_capacity(dto.managed_files.len());
    let mut pending_provider_files = Vec::new();
    let mut embedded_files = Vec::new();
    let mut referenced_objects = BTreeSet::new();

    for file in &dto.managed_files {
        match normalize_managed_file(index, root_prefix, file, &cancellation)? {
            NormalizedAcquisition::Remote(managed) => managed_files.push(managed),
            NormalizedAcquisition::Provider(pending) => pending_provider_files.push(pending),
            NormalizedAcquisition::Embedded(embedded, object_entry) => {
                referenced_objects.insert(object_entry);
                embedded_files.push(embedded);
            }
        }
    }

    let mut listed_seeds = BTreeSet::new();
    let mut seed_entries = Vec::with_capacity(dto.seed_files.len());
    for seed in &dto.seed_files {
        let entry = normalize_seed_file(index, root_prefix, seed, &cancellation)?;
        listed_seeds.insert(entry.0.clone());
        seed_entries.push(entry.1);
    }

    audit_unlisted_payload(
        index,
        &manifest_name,
        root_prefix,
        &referenced_objects,
        &listed_seeds,
    )?;

    let diagnostics = vec![PackDiagnostic::new(
        PackDiagnosticCode::SourceHasNoAuthenticityProof,
        "pack source identity is limited to the observed content digest",
    )?];

    let mut normalized = NormalizedModpack::new(
        PackFormat::Graphene,
        metadata,
        runtime,
        managed_files,
        pending_provider_files,
        embedded_files,
        seed_entries,
        Vec::new(),
        diagnostics,
    )?;
    normalized.sort_for_stability();
    Ok(normalized)
}

enum NormalizedAcquisition {
    Remote(NormalizedPackFile),
    Provider(crate::model::file::PendingProviderFile),
    Embedded(EmbeddedPackFile, String),
}

fn normalize_managed_file<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    root_prefix: &str,
    file: &GrapheneManagedFileDto,
    cancellation: &CancellationToken,
) -> Result<NormalizedAcquisition, PackError> {
    let destination = crate::archive::PackPath::normalize(&file.destination)?;
    let digest = parse_sha256(&file.sha256)?;
    if file.size == 0 {
        return Err(PackError::manifest(
            "managed file declares a zero-byte payload",
        ));
    }
    let hint = hint_for(&destination);

    let strategies = usize::from(!file.sources.is_empty())
        + usize::from(file.embedded_object.is_some())
        + usize::from(file.content_provenance.is_some());
    if strategies != 1 {
        return Err(PackError::manifest(
            "managed file must declare exactly one acquisition strategy",
        ));
    }

    if let Some(embedded_hex) = &file.embedded_object {
        return normalize_embedded(
            index,
            root_prefix,
            embedded_hex,
            digest,
            file.size,
            destination,
            hint,
            cancellation,
        )
        .map(|(embedded, entry)| NormalizedAcquisition::Embedded(embedded, entry));
    }

    if let Some(provenance) = &file.content_provenance {
        let provider_ref = normalize_provenance(provenance)?;
        let pending =
            crate::model::file::PendingProviderFile::new(provider_ref, FileSelection::Required)?;
        return Ok(NormalizedAcquisition::Provider(pending));
    }

    if file.sources.len() > MAX_SOURCES_PER_FILE {
        return Err(PackError::new(
            ErrorCode::PackSourceTooLarge,
            "managed source list exceeds its bound",
        ));
    }
    let mut sources = Vec::with_capacity(file.sources.len());
    for raw in &file.sources {
        // A portable pack may reference any CDN; structural persisted-URL policy still applies
        // (HTTPS only, no userinfo, no fragment, no secret-bearing query keys).
        sources.push(ManagedSource::parse(raw, |_| true)?);
    }
    if sources.is_empty() {
        return Err(PackError::new(
            ErrorCode::PackArtifactUnverifiable,
            "remote managed file has neither a safe source nor provider identity",
        ));
    }
    let integrity = ArtifactIntegrity::none().with_sha256(digest);
    Ok(NormalizedAcquisition::Remote(NormalizedPackFile::new(
        destination,
        FileSelection::Required,
        file.size,
        integrity,
        sources,
        None,
        hint,
    )?))
}

#[allow(clippy::too_many_arguments)]
fn normalize_embedded<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    root_prefix: &str,
    embedded_hex: &str,
    digest: Sha256Digest,
    size: u64,
    destination: crate::archive::PackPath,
    hint: Option<ContentHint>,
    cancellation: &CancellationToken,
) -> Result<(EmbeddedPackFile, String), PackError> {
    let declared = parse_sha256(embedded_hex)?;
    if declared != digest {
        return Err(PackError::manifest(
            "embedded object identity does not match the file digest",
        ));
    }
    // The object path is derived from its SHA-256, never from an untrusted filename.
    let hex = declared.to_string();
    let entry = format!("{root_prefix}{OBJECTS_PREFIX}{}/{}", &hex[..2], hex);
    let observed = index.stream_entry_hashed(&entry, &mut sink(), cancellation)?;
    if observed.size != size {
        return Err(PackError::manifest(
            "embedded object size does not match its declaration",
        ));
    }
    if observed.sha256 != digest {
        return Err(PackError::manifest(
            "embedded object bytes do not match their declared SHA-256",
        ));
    }
    Ok((
        EmbeddedPackFile::new(entry.clone(), destination, size, digest, hint),
        entry,
    ))
}

fn normalize_provenance(
    provenance: &super::dto::GrapheneProvenanceDto,
) -> Result<ProviderFileRef, PackError> {
    if provenance.provider.as_str() != "curseforge" {
        return Err(PackError::new(
            ErrorCode::PackRuntimeUnsupported,
            format!(
                "pack provenance references unsupported provider '{}'",
                provenance.provider
            ),
        ));
    }
    validate_provider_id("project id", &provenance.project_id)?;
    validate_provider_id("version id", &provenance.version_id)?;
    Ok(ProviderFileRef::CurseForge {
        project_id: provenance.project_id.clone(),
        file_id: provenance.version_id.clone(),
    })
}

fn validate_provider_id(field: &str, value: &str) -> Result<(), PackError> {
    if value.is_empty()
        || value.chars().count() > MAX_PROVIDER_ID_CHARS
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(PackError::manifest(format!(
            "provider {field} is missing or malformed"
        )));
    }
    Ok(())
}

fn normalize_seed_file<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    root_prefix: &str,
    seed: &GrapheneSeedFileDto,
    cancellation: &CancellationToken,
) -> Result<(String, NormalizedSeedEntry), PackError> {
    let destination = crate::archive::PackPath::normalize(&seed.destination)?;
    let digest = parse_sha256(&seed.sha256)?;
    if seed.size == 0 {
        return Err(PackError::manifest(
            "seed file declares a zero-byte payload",
        ));
    }
    // The archive entry is derived from the destination; arbitrary entry names are rejected so
    // the manifest cannot alias one payload into two identities.
    let expected_entry = format!("{root_prefix}{SEED_PREFIX}{}", destination.as_str());
    if seed.archive_entry != expected_entry {
        return Err(PackError::manifest(
            "seed archive entry does not match its derived path",
        ));
    }
    let observed = index.stream_entry_hashed(&seed.archive_entry, &mut sink(), cancellation)?;
    if observed.size != seed.size {
        return Err(PackError::manifest(
            "seed file size does not match its declaration",
        ));
    }
    if observed.sha256 != digest {
        return Err(PackError::manifest(
            "seed file bytes do not match their declared SHA-256",
        ));
    }
    Ok((
        seed.archive_entry.clone(),
        NormalizedSeedEntry::new(
            seed.archive_entry.clone(),
            destination,
            SeedLayer::base(),
            seed.size,
            digest,
        ),
    ))
}

/// Rejects any regular archive file that is neither the manifest nor a listed/referenced payload.
fn audit_unlisted_payload<R: Read + Seek>(
    index: &PackArchiveIndex<R>,
    manifest_name: &str,
    root_prefix: &str,
    referenced_objects: &BTreeSet<String>,
    listed_seeds: &BTreeSet<String>,
) -> Result<(), PackError> {
    let objects_prefix = format!("{root_prefix}{OBJECTS_PREFIX}");
    let seed_prefix = format!("{root_prefix}{SEED_PREFIX}");
    for entry in index.entries() {
        if !entry.is_regular || entry.name == manifest_name {
            continue;
        }
        if entry.name.starts_with(&objects_prefix) {
            if !referenced_objects.contains(&entry.name) {
                return Err(PackError::manifest(format!(
                    "unlisted embedded object payload: {}",
                    entry.name
                )));
            }
        } else if entry.name.starts_with(&seed_prefix) {
            if !listed_seeds.contains(&entry.name) {
                return Err(PackError::manifest(format!(
                    "unlisted seed payload: {}",
                    entry.name
                )));
            }
        } else {
            return Err(PackError::manifest(format!(
                "unlisted regular payload outside reserved areas: {}",
                entry.name
            )));
        }
    }
    Ok(())
}

fn read_metadata(pack: &super::dto::GraphenePackInfoDto) -> Result<PackMetadata, PackError> {
    PackMetadata::new(
        pack.name.clone(),
        pack.version.clone(),
        pack.summary.clone(),
        Vec::new(),
    )
}

fn read_runtime(
    runtime: &super::dto::GrapheneRuntimeDto,
) -> Result<PackRuntimeRequirement, PackError> {
    let primary_loader = match &runtime.primary_loader {
        None => None,
        Some(loader) => {
            let kind = match loader.kind.as_str() {
                "fabric" => graphene_minecraft::LoaderKind::Fabric,
                "forge" => graphene_minecraft::LoaderKind::Forge,
                "neoforge" => graphene_minecraft::LoaderKind::NeoForge,
                other => {
                    return Err(PackError::new(
                        ErrorCode::PackRuntimeUnsupported,
                        format!("pack requires unsupported primary loader '{other}'"),
                    ));
                }
            };
            Some(PackLoaderRequirement::new(
                kind,
                loader.exact_version.clone(),
            )?)
        }
    };
    PackRuntimeRequirement::new(runtime.minecraft_version.clone(), primary_loader)
}

fn hint_for(destination: &crate::archive::PackPath) -> Option<ContentHint> {
    let components: Vec<&str> = destination.components().collect();
    match components.first().copied() {
        Some("mods") => Some(ContentHint::Mod),
        Some("resourcepacks") => Some(ContentHint::ResourcePack),
        Some("shaderpacks") => Some(ContentHint::ShaderPack),
        _ => None,
    }
}

fn parse_sha256(value: &str) -> Result<Sha256Digest, PackError> {
    value
        .parse::<Sha256Digest>()
        .map_err(|_| PackError::manifest("declared SHA-256 hash is missing or malformed"))
}
