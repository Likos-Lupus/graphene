use super::dto::{CurseForgeFileEntry, CurseForgeManifestDocument, CurseForgeMinecraftSection};
use crate::archive::limits::{MAX_MANAGED_FILES, MAX_SEED_ENTRIES};
use crate::archive::{PackArchiveIndex, PackPath};
use crate::error::PackError;
use crate::model::{
    ContentHint, EmbeddedPackFile, FileSelection, NormalizedModpack, NormalizedSeedEntry,
    OptionalSelectionPolicy, PackDiagnostic, PackDiagnosticCode, PackFormat, PackLoaderRequirement,
    PackMetadata, PackOptionalChoice, PackRuntimeRequirement, PendingProviderFile, ProviderFileRef,
    SeedLayer,
};
use graphene_core::{CancellationToken, ErrorCode};
use graphene_minecraft::LoaderKind;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, sink};

type ProviderFiles = (
    Vec<PendingProviderFile>,
    Vec<PackOptionalChoice>,
    Vec<PackDiagnostic>,
);

/// Normalizes a detected CurseForge export into the Graphene-owned pack model.
pub fn normalize<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    root_prefix: &str,
    optional_policy: &OptionalSelectionPolicy,
) -> Result<NormalizedModpack, PackError> {
    let manifest_name = format!("{root_prefix}{}", super::MANIFEST_NAME);
    let bytes = index.read_manifest(&manifest_name)?;
    let dto: CurseForgeManifestDocument = serde_json::from_slice(&bytes)
        .map_err(|_| PackError::manifest("manifest.json does not match CurseForge format 1"))?;

    validate_manifest_identity(&dto)?;
    let metadata = PackMetadata::new(
        dto.name.clone(),
        Some(dto.version.clone()),
        None,
        vec![dto.author.clone()],
    )?;
    let runtime = normalize_runtime(&dto.minecraft)?;
    let (pending_provider_files, optional_choices, mut diagnostics) =
        normalize_provider_files(&dto.files, optional_policy)?;
    let (embedded_files, seed_entries) = collect_overrides(index, root_prefix)?;

    diagnostics.push(PackDiagnostic::new(
        PackDiagnosticCode::SourceHasNoAuthenticityProof,
        "pack source identity is limited to the observed content digest",
    )?);
    if !embedded_files.is_empty() {
        diagnostics.push(PackDiagnostic::new(
            PackDiagnosticCode::EmbeddedModHasNoProviderIdentity,
            "embedded CurseForge override mods have no provider identity",
        )?);
    }

    let mut normalized = NormalizedModpack::new(
        PackFormat::CurseForge,
        metadata,
        runtime,
        Vec::new(),
        pending_provider_files,
        embedded_files,
        seed_entries,
        optional_choices,
        diagnostics,
    )?;
    normalized.sort_for_stability();
    Ok(normalized)
}

fn validate_manifest_identity(dto: &CurseForgeManifestDocument) -> Result<(), PackError> {
    if dto.manifest_type != "minecraftModpack" {
        return Err(PackError::manifest(
            "manifest.json has an unsupported manifestType",
        ));
    }
    if dto.manifest_version != 1 {
        return Err(PackError::manifest(format!(
            "unsupported CurseForge manifest version {}",
            dto.manifest_version
        )));
    }
    if dto.files.len() > MAX_MANAGED_FILES {
        return Err(PackError::new(
            ErrorCode::PackSourceTooLarge,
            "CurseForge file count exceeds its bound",
        ));
    }
    if let Some(project_id) = dto.project_id {
        validate_id(project_id, "pack projectID")?;
    }
    match dto.overrides.as_deref() {
        None | Some("overrides") => Ok(()),
        Some(_) => Err(PackError::manifest(
            "CurseForge overrides must use the exact 'overrides' root",
        )),
    }
}

fn normalize_runtime(
    minecraft: &CurseForgeMinecraftSection,
) -> Result<PackRuntimeRequirement, PackError> {
    validate_exact_minecraft_version(&minecraft.version)?;
    if minecraft.mod_loaders.is_empty() {
        return PackRuntimeRequirement::new(minecraft.version.clone(), None);
    }

    let primary_count = minecraft
        .mod_loaders
        .iter()
        .filter(|loader| loader.primary)
        .count();
    if primary_count != 1 {
        return Err(PackError::new(
            ErrorCode::PackRuntimeUnsupported,
            "CurseForge pack must declare exactly one primary loader",
        ));
    }
    if minecraft.mod_loaders.len() != 1 {
        return Err(PackError::new(
            ErrorCode::PackRuntimeUnsupported,
            "CurseForge pack declares unsupported secondary loaders",
        ));
    }

    let loader = &minecraft.mod_loaders[0];
    let (kind, version) = parse_loader_id(&loader.id, &minecraft.version)?;
    let primary = PackLoaderRequirement::new(kind, version)?;
    PackRuntimeRequirement::new(minecraft.version.clone(), Some(primary))
}

fn validate_exact_minecraft_version(version: &str) -> Result<(), PackError> {
    let lower = version.to_ascii_lowercase();
    let wildcard = lower
        .split(['.', '-', '_', '+'])
        .any(|part| matches!(part, "x" | "latest" | "recommended" | "*") || part.is_empty());
    if wildcard
        || matches!(
            lower.as_str(),
            "latest" | "recommended" | "release" | "snapshot"
        )
    {
        return Err(PackError::manifest(
            "CurseForge pack does not declare an exact Minecraft version",
        ));
    }
    PackRuntimeRequirement::new(version.to_owned(), None).map(|_| ())
}

fn parse_loader_id(id: &str, minecraft: &str) -> Result<(LoaderKind, String), PackError> {
    let (kind, raw_version) = if let Some(version) = id.strip_prefix("fabric-") {
        (LoaderKind::Fabric, version)
    } else if let Some(version) = id.strip_prefix("neoforge-") {
        (LoaderKind::NeoForge, version)
    } else if let Some(version) = id.strip_prefix("forge-") {
        (LoaderKind::Forge, version)
    } else if id.starts_with("quilt-") {
        return Err(PackError::new(
            ErrorCode::PackRuntimeUnsupported,
            "CurseForge pack requires the unsupported Quilt loader",
        ));
    } else {
        return Err(PackError::new(
            ErrorCode::PackRuntimeUnsupported,
            "CurseForge pack declares an unknown primary loader",
        ));
    };

    let version = if kind == LoaderKind::Forge {
        raw_version
            .strip_prefix(&format!("{minecraft}-"))
            .unwrap_or(raw_version)
    } else {
        raw_version
    };
    if !is_exact_loader_version(version) {
        return Err(PackError::new(
            ErrorCode::PackRuntimeUnsupported,
            "CurseForge loader declaration has no valid exact version",
        ));
    }
    Ok((kind, version.to_owned()))
}

fn is_exact_loader_version(version: &str) -> bool {
    let lower = version.to_ascii_lowercase();
    !version.is_empty()
        && version.len() <= 64
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+' | '~'))
        && !matches!(
            lower.as_str(),
            "latest" | "recommended" | "stable" | "release" | "snapshot"
        )
        && !lower
            .split(['.', '-', '_', '+'])
            .any(|part| matches!(part, "x" | "latest" | "recommended") || part.is_empty())
}

fn normalize_provider_files(
    files: &[CurseForgeFileEntry],
    policy: &OptionalSelectionPolicy,
) -> Result<ProviderFiles, PackError> {
    let mut seen = BTreeSet::new();
    let mut pending = Vec::with_capacity(files.len());
    let mut choices = Vec::new();
    let mut diagnostics = Vec::new();

    for file in files {
        validate_id(file.project_id, "file projectID")?;
        validate_id(file.file_id, "fileID")?;
        let project_id = file.project_id.to_string();
        let file_id = file.file_id.to_string();
        let stable_id = format!("curseforge:{project_id}:{file_id}");
        if !seen.insert(stable_id.clone()) {
            return Err(PackError::manifest(
                "CurseForge manifest repeats an exact project/file pair",
            ));
        }

        let selection = if file.required {
            FileSelection::Required
        } else {
            choices.push(PackOptionalChoice::unresolved(
                stable_id.clone(),
                format!("CurseForge project {project_id}, file {file_id}"),
                None,
                false,
            )?);
            match policy {
                OptionalSelectionPolicy::RequiredOnly => {
                    diagnostics.push(optional_skipped(
                        "an optional CurseForge file was not selected",
                    )?);
                    continue;
                }
                OptionalSelectionPolicy::IncludeAllOptional => FileSelection::Optional {
                    choice_id: stable_id.clone(),
                },
                OptionalSelectionPolicy::Explicit(selected) => {
                    if !selected.contains(&stable_id) {
                        diagnostics.push(optional_skipped(
                            "an explicitly unselected optional CurseForge file was skipped",
                        )?);
                        continue;
                    }
                    FileSelection::Optional {
                        choice_id: stable_id.clone(),
                    }
                }
            }
        };

        let provider_ref = ProviderFileRef::CurseForge {
            project_id,
            file_id,
        };
        pending.push(PendingProviderFile::new(provider_ref, selection)?);
    }

    Ok((pending, choices, diagnostics))
}

fn validate_id(value: u64, field: &str) -> Result<(), PackError> {
    if value == 0 || u32::try_from(value).is_err() {
        return Err(PackError::manifest(format!(
            "CurseForge {field} must be a nonzero u32"
        )));
    }
    Ok(())
}

fn optional_skipped(message: &str) -> Result<PackDiagnostic, PackError> {
    PackDiagnostic::new(PackDiagnosticCode::OptionalFileNotSelected, message)
}

fn collect_overrides<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    root_prefix: &str,
) -> Result<(Vec<EmbeddedPackFile>, Vec<NormalizedSeedEntry>), PackError> {
    let prefix = format!("{root_prefix}overrides/");
    let entries: Vec<(String, bool, bool, bool)> = index
        .entries()
        .iter()
        .filter(|entry| entry.name.starts_with(&prefix) && entry.name != prefix)
        .map(|entry| {
            (
                entry.name.clone(),
                entry.is_regular,
                entry.is_directory,
                entry.is_symlink,
            )
        })
        .collect();
    if entries.len() > MAX_SEED_ENTRIES {
        return Err(PackError::new(
            ErrorCode::PackSourceTooLarge,
            "CurseForge override entry count exceeds its bound",
        ));
    }

    let mut seen = BTreeMap::new();
    let mut embedded = Vec::new();
    let mut seeds = Vec::new();
    for (archive_entry, is_regular, is_directory, is_symlink) in entries {
        let relative = &archive_entry[prefix.len()..];
        let path_text = relative.strip_suffix('/').unwrap_or(relative);
        let destination = PackPath::normalize(path_text)?;
        if seen
            .insert(destination.collision_key(), destination.as_str().to_owned())
            .is_some()
        {
            return Err(PackError::path(
                "CurseForge overrides contain duplicate or case-colliding paths",
            ));
        }
        if is_directory && !is_symlink {
            continue;
        }
        if !is_regular {
            return Err(PackError::archive(
                "CurseForge overrides contain a symlink or special archive entry",
            ));
        }

        let observed =
            index.stream_entry_hashed(&archive_entry, &mut sink(), &CancellationToken::new())?;
        if crate::model::modpack::is_mod_destination(&destination) {
            embedded.push(EmbeddedPackFile::new(
                archive_entry,
                destination,
                observed.size,
                observed.sha256,
                Some(ContentHint::Mod),
            ));
        } else {
            seeds.push(NormalizedSeedEntry::new(
                archive_entry,
                destination,
                SeedLayer::base(),
                observed.size,
                observed.sha256,
            ));
        }
    }
    Ok((embedded, seeds))
}
