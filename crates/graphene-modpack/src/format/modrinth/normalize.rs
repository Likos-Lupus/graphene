use super::dto::{ModrinthEnvValue, ModrinthFileDto, ModrinthIndexDto};
use crate::archive::PackArchiveIndex;
use crate::archive::limits::{MAX_DECLARED_MANAGED_BYTES, MAX_DISPLAY_STRING_CHARS};
use crate::error::PackError;
use crate::model::file::{ContentHint, FileSelection, ManagedSource, NormalizedPackFile};
use crate::model::format::PackFormat;
use crate::model::metadata::PackMetadata;
use crate::model::modpack::{NormalizedModpack, PackDiagnostic, PackDiagnosticCode};
use crate::model::runtime::{PackLoaderRequirement, PackRuntimeRequirement};
use crate::model::seed::{NormalizedSeedEntry, SeedLayer};
use crate::model::selection::PackOptionalChoice;
use graphene_core::{ArtifactIntegrity, CancellationToken, ErrorCode, Sha1Digest, Sha512Digest};
use std::io::{Read, Seek, sink};

/// Download hosts allowed by the default Modrinth pack-source policy.
pub const DEFAULT_ALLOWED_DOWNLOAD_HOSTS: [&str; 1] = ["cdn.modrinth.com"];

const KNOWN_RUNTIME_DEPENDENCIES: [&str; 4] = ["minecraft", "forge", "neoforge", "fabric-loader"];

/// Normalizes a detected `.mrpack` archive into the Graphene-owned pack model.
///
/// `optional_policy` decides how optional client files are represented after normalization:
/// required files always stay managed; optional files become choices unless the host policy
/// pre-selects or excludes them.
pub fn normalize<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    root_prefix: &str,
    optional_policy: &crate::model::OptionalSelectionPolicy,
) -> Result<NormalizedModpack, PackError> {
    let manifest_name = format!("{root_prefix}modrinth.index.json");
    let bytes = index.read_manifest(&manifest_name)?;
    let dto: ModrinthIndexDto = serde_json::from_slice(&bytes)
        .map_err(|_| PackError::manifest("modrinth.index.json does not match format v1"))?;

    if dto.format_version != 1 {
        return Err(PackError::manifest(format!(
            "unsupported Modrinth pack format version {}",
            dto.format_version
        )));
    }
    if dto.game != "minecraft" {
        return Err(PackError::manifest(
            "Modrinth pack targets an unsupported game",
        ));
    }

    let metadata = read_metadata(&dto)?;
    let runtime = read_runtime(&dto.dependencies)?;

    if dto.files.len() > super::super::super::archive::limits::MAX_MANAGED_FILES {
        return Err(PackError::new(
            ErrorCode::PackSourceTooLarge,
            "declared file count exceeds its bound",
        ));
    }

    let mut diagnostics = Vec::new();
    let mut managed_files = Vec::with_capacity(dto.files.len());
    let mut optional_choices = Vec::new();
    let mut total_declared = 0_u64;

    for file in &dto.files {
        total_declared += file.file_size;
        if total_declared > MAX_DECLARED_MANAGED_BYTES {
            return Err(PackError::new(
                ErrorCode::PackSourceTooLarge,
                "declared download bytes exceed their bound",
            ));
        }
        if let Some(managed) = normalize_file(
            file,
            optional_policy,
            &mut optional_choices,
            &mut diagnostics,
        )? {
            managed_files.push(managed);
        }
    }

    // Deterministic override layering: overrides/ then client-overrides/.
    let (overrides, _) = collect_seed_entries(index, root_prefix, "overrides", SeedLayer::base())?;
    let (client_overrides, _) = collect_seed_entries(
        index,
        root_prefix,
        "client-overrides",
        SeedLayer::client_override(),
    )?;
    let mut seed_entries = overrides;
    seed_entries.extend(client_overrides);

    let server_overrides = count_layer_entries(index, root_prefix, "server-overrides");
    if server_overrides > 0 {
        diagnostics.push(PackDiagnostic::new(
            PackDiagnosticCode::ServerOnlyDataSkipped,
            format!(
                "server-overrides contains {server_overrides} files skipped for a client import"
            ),
        )?);
    }
    diagnostics.push(PackDiagnostic::new(
        PackDiagnosticCode::SourceHasNoAuthenticityProof,
        "pack source identity is limited to the observed content digest",
    )?);
    diagnostics.shrink_to_fit();

    let mut normalized = NormalizedModpack::new(
        PackFormat::Modrinth,
        metadata,
        runtime,
        managed_files,
        Vec::new(),
        Vec::new(),
        seed_entries,
        optional_choices,
        diagnostics,
    )?;
    normalized.sort_for_stability();
    Ok(normalized)
}

fn normalize_file(
    file: &ModrinthFileDto,
    policy: &crate::model::OptionalSelectionPolicy,
    optional_choices: &mut Vec<PackOptionalChoice>,
    diagnostics: &mut Vec<PackDiagnostic>,
) -> Result<Option<NormalizedPackFile>, PackError> {
    let destination = crate::archive::PackPath::normalize(&file.path)?;
    let sha1 = parse_sha1(&file.hashes.sha1)?;
    let sha512 = parse_sha512(&file.hashes.sha512)?;
    let integrity = ArtifactIntegrity::none()
        .with_sha1(sha1)
        .with_sha512(sha512);

    if file.downloads.is_empty() || file.downloads.len() > super::dto::MAX_DOWNLOAD_URLS_PER_FILE {
        return Err(PackError::new(
            ErrorCode::PackArtifactUnverifiable,
            "declared file has no usable download list",
        ));
    }
    let mut sources = Vec::with_capacity(file.downloads.len());
    for raw in &file.downloads {
        match ManagedSource::parse(raw, allowed_host) {
            Ok(source) => sources.push(source),
            Err(_) => continue, // untrusted hosts are filtered by policy, not trusted blindly
        }
    }
    if sources.is_empty() {
        return Err(PackError::new(
            ErrorCode::PackArtifactSourceUnsafe,
            "every declared download host failed the pack source policy",
        ));
    }

    let hint = hint_for(&destination);
    let selection = match client_side_value(file) {
        Some(ModrinthEnvValue::Unsupported) => {
            let message = if server_side_value(file).is_some_and(|value| {
                matches!(
                    value,
                    ModrinthEnvValue::Required | ModrinthEnvValue::Optional
                )
            }) {
                "a declared server-only pack file was excluded from the client import"
            } else {
                "a declared pack file is unsupported on the client and was excluded"
            };
            diagnostics.push(PackDiagnostic::new(
                PackDiagnosticCode::ServerOnlyDataSkipped,
                message,
            )?);
            return Ok(None);
        }
        Some(ModrinthEnvValue::Optional) => {
            let choice_id = format!("mrpack:{}", destination.as_str());
            let choice = PackOptionalChoice::new(
                choice_id.clone(),
                destination.clone(),
                destination.file_name().to_owned(),
                file.file_size,
                None,
                false,
            )?;
            optional_choices.push(choice);
            match policy {
                crate::model::OptionalSelectionPolicy::RequiredOnly => {
                    diagnostics.push(PackDiagnostic::new(
                        PackDiagnosticCode::OptionalFileNotSelected,
                        "an optional pack file was not selected",
                    )?);
                    return Ok(None);
                }
                crate::model::OptionalSelectionPolicy::IncludeAllOptional => {
                    FileSelection::Optional { choice_id }
                }
                crate::model::OptionalSelectionPolicy::Explicit(selected) => {
                    if !selected.contains(&choice_id) {
                        diagnostics.push(PackDiagnostic::new(
                            PackDiagnosticCode::OptionalFileNotSelected,
                            "an explicitly unselected optional pack file was skipped",
                        )?);
                        return Ok(None);
                    }
                    FileSelection::Optional { choice_id }
                }
            }
        }
        _ => FileSelection::Required,
    };

    Ok(Some(NormalizedPackFile::new(
        destination,
        selection,
        file.file_size,
        integrity,
        sources,
        None,
        hint,
    )?))
}

fn client_side_value(file: &ModrinthFileDto) -> Option<ModrinthEnvValue> {
    file.env.as_ref().and_then(|env| env.client)
}

fn server_side_value(file: &ModrinthFileDto) -> Option<ModrinthEnvValue> {
    file.env.as_ref().and_then(|env| env.server)
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

fn read_metadata(dto: &ModrinthIndexDto) -> Result<PackMetadata, PackError> {
    let name = dto
        .name
        .clone()
        .unwrap_or_else(|| "Modrinth modpack".to_owned());
    let version = dto.version_id.clone();
    let summary = dto.summary.clone();
    if version
        .as_ref()
        .is_some_and(|v| v.chars().count() > MAX_DISPLAY_STRING_CHARS)
    {
        return Err(PackError::manifest("versionId exceeds its display bound"));
    }
    if summary
        .as_ref()
        .is_some_and(|s| s.chars().count() > MAX_DISPLAY_STRING_CHARS)
    {
        return Err(PackError::manifest("summary exceeds its display bound"));
    }
    PackMetadata::new(name, version, summary, Vec::new())
}

fn read_runtime(
    dependencies: &std::collections::BTreeMap<String, String>,
) -> Result<PackRuntimeRequirement, PackError> {
    if dependencies.len() > super::dto::MAX_DEPENDENCIES {
        return Err(PackError::manifest("dependency list exceeds its bound"));
    }
    let minecraft = dependencies.get("minecraft").ok_or_else(|| {
        PackError::manifest("Modrinth pack declares no exact Minecraft dependency")
    })?;
    if minecraft.trim().is_empty() {
        return Err(PackError::manifest("Minecraft dependency is empty"));
    }

    let mut primary: Option<PackLoaderRequirement> = None;
    for key in ["forge", "neoforge", "fabric-loader"] {
        let Some(version) = dependencies.get(key) else {
            continue;
        };
        let kind = match key {
            "forge" => graphene_minecraft::LoaderKind::Forge,
            "neoforge" => graphene_minecraft::LoaderKind::NeoForge,
            _ => graphene_minecraft::LoaderKind::Fabric,
        };
        let requirement = PackLoaderRequirement::new(kind, version.clone())?;
        if primary.replace(requirement).is_some() {
            return Err(PackError::new(
                ErrorCode::PackRuntimeUnsupported,
                "pack declares more than one primary loader",
            ));
        }
    }

    for key in dependencies.keys() {
        if KNOWN_RUNTIME_DEPENDENCIES.contains(&key.as_str()) {
            continue;
        }
        if key == "quilt-loader" {
            return Err(PackError::new(
                ErrorCode::PackRuntimeUnsupported,
                "pack requires the Quilt loader which is not a supported component",
            ));
        }
        return Err(PackError::manifest(format!(
            "pack declares unknown runtime dependency '{key}'"
        )));
    }

    PackRuntimeRequirement::new(minecraft.clone(), primary)
}

fn collect_seed_entries<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    root_prefix: &str,
    layer_dir: &str,
    layer: SeedLayer,
) -> Result<(Vec<NormalizedSeedEntry>, usize), PackError> {
    let prefix = format!("{root_prefix}{layer_dir}/");
    let names: Vec<String> = index
        .entries()
        .iter()
        .filter(|entry| entry.is_regular && entry.name.starts_with(&prefix))
        .map(|entry| entry.name.clone())
        .collect();

    let mut skipped = 0_usize;
    let mut entries = Vec::with_capacity(names.len());
    for name in &names {
        let relative = &name[prefix.len()..];
        if relative.is_empty() {
            continue;
        }
        let Ok(destination) = crate::archive::PackPath::normalize(relative) else {
            // Unsafe payload paths are rejected during planning validation; counting them here
            // keeps inspection bounded without failing the whole scan early.
            skipped += 1;
            continue;
        };
        let observed = index.stream_entry_hashed(name, &mut sink(), &CancellationToken::new())?;
        entries.push(NormalizedSeedEntry::new(
            name.clone(),
            destination,
            layer,
            observed.size,
            observed.sha256,
        ));
    }
    Ok((entries, skipped))
}

fn count_layer_entries<R: Read + Seek>(
    index: &PackArchiveIndex<R>,
    root_prefix: &str,
    layer_dir: &str,
) -> usize {
    let prefix = format!("{root_prefix}{layer_dir}/");
    index
        .entries()
        .iter()
        .filter(|entry| {
            entry.is_regular && entry.name.starts_with(&prefix) && entry.name.len() > prefix.len()
        })
        .count()
}

fn allowed_host(host: &str) -> bool {
    DEFAULT_ALLOWED_DOWNLOAD_HOSTS.contains(&host)
}

fn parse_sha1(value: &str) -> Result<Sha1Digest, PackError> {
    value
        .parse::<Sha1Digest>()
        .map_err(|_| PackError::manifest("declared SHA-1 hash is missing or malformed"))
}

fn parse_sha512(value: &str) -> Result<Sha512Digest, PackError> {
    value
        .parse::<Sha512Digest>()
        .map_err(|_| PackError::manifest("declared SHA-512 hash is missing or malformed"))
}
