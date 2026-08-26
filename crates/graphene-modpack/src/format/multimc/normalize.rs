use super::dto::{MAX_COMPONENTS, MultiMcPackDocument, MultiMcPatchDocument};
use crate::archive::limits::{MAX_DISPLAY_STRING_CHARS, MAX_SEED_ENTRIES};
use crate::archive::{PackArchiveIndex, PackPath};
use crate::error::PackError;
use crate::model::{NormalizedSeedEntry, PackLoaderRequirement, PackMetadata};
use crate::model::{PackRuntimeRequirement, SeedLayer};
use graphene_core::{CancellationToken, ErrorCode, Sha256Digest};
use graphene_minecraft::LoaderKind;
use std::collections::BTreeMap;
use std::io::{Read, Seek, sink};

const MINECRAFT_UID: &str = "net.minecraft";
const FABRIC_UID: &str = "net.fabricmc.fabric-loader";
const FORGE_UID: &str = "net.minecraftforge";
const NEOFORGE_UID: &str = "net.neoforged";
const AUXILIARY_UIDS: [&str; 3] = ["org.lwjgl", "org.lwjgl3", "net.fabricmc.intermediary"];
const REDUNDANT_PATCH_FIELDS: [&str; 8] = [
    "cachedName",
    "cachedRequires",
    "cachedVersion",
    "cachedVolatile",
    "dependencyOnly",
    "important",
    "name",
    "order",
];

/// One mod JAR embedded in the exported instance and identified independently from seed files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedMod {
    archive_entry: String,
    destination: PackPath,
    size: u64,
    sha256: Sha256Digest,
}

impl EmbeddedMod {
    #[must_use]
    pub fn archive_entry(&self) -> &str {
        &self.archive_entry
    }

    #[must_use]
    pub fn destination(&self) -> &PackPath {
        &self.destination
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn sha256(&self) -> &Sha256Digest {
        &self.sha256
    }
}

/// Graphene-owned normalized result for a standard Prism Launcher / MultiMC export.
#[derive(Debug, Clone)]
pub struct NormalizedMultiMcPack {
    metadata: PackMetadata,
    runtime: PackRuntimeRequirement,
    embedded_mods: Vec<EmbeddedMod>,
    seed_entries: Vec<NormalizedSeedEntry>,
}

impl NormalizedMultiMcPack {
    #[must_use]
    pub const fn metadata(&self) -> &PackMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn runtime(&self) -> &PackRuntimeRequirement {
        &self.runtime
    }

    #[must_use]
    pub fn embedded_mods(&self) -> &[EmbeddedMod] {
        &self.embedded_mods
    }

    #[must_use]
    pub fn seed_entries(&self) -> &[NormalizedSeedEntry] {
        &self.seed_entries
    }
}

/// Normalizes a detected standard Prism Launcher / MultiMC export.
///
/// Foreign launcher caches and configuration stay outside the result. Runtime components are
/// reduced to exact Graphene requirements, while embedded mod JARs remain distinct from mutable
/// instance seed files.
pub fn normalize<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    root_prefix: &str,
) -> Result<NormalizedMultiMcPack, PackError> {
    let manifest_name = format!("{root_prefix}{}", super::PACK_NAME);
    let bytes = index.read_manifest(&manifest_name)?;
    let dto: MultiMcPackDocument = serde_json::from_slice(&bytes)
        .map_err(|_| PackError::manifest("mmc-pack.json does not match format version 1"))?;
    if dto.format_version != 1 {
        return Err(PackError::manifest(format!(
            "unsupported mmc-pack.json format version {}",
            dto.format_version
        )));
    }
    if dto
        .extra
        .keys()
        .any(|key| key.eq_ignore_ascii_case("jarMods"))
    {
        return Err(PackError::new(
            ErrorCode::PackRuntimeUnsupported,
            "mmc-pack.json declares jarmods that cannot be imported",
        ));
    }
    if !dto.extra.is_empty() {
        return Err(PackError::new(
            ErrorCode::PackRuntimeUnsupported,
            "mmc-pack.json contains unsupported foreign runtime metadata",
        ));
    }

    let (runtime, components) = normalize_runtime(&dto)?;
    reject_jarmods(index, root_prefix)?;
    validate_patches(index, root_prefix, &components)?;
    let metadata = read_metadata(index, root_prefix)?;
    let content_root = select_content_root(index, root_prefix)?;
    let (mut embedded_mods, mut seed_entries) =
        collect_content(index, &content_root, &CancellationToken::new())?;

    embedded_mods.sort_by(|a, b| a.destination.as_str().cmp(b.destination.as_str()));
    seed_entries.sort_by(|a, b| a.destination().as_str().cmp(b.destination().as_str()));

    Ok(NormalizedMultiMcPack {
        metadata,
        runtime,
        embedded_mods,
        seed_entries,
    })
}

fn normalize_runtime(
    dto: &MultiMcPackDocument,
) -> Result<(PackRuntimeRequirement, BTreeMap<String, String>), PackError> {
    if dto.components.len() > MAX_COMPONENTS {
        return Err(PackError::new(
            ErrorCode::PackSourceTooLarge,
            "MultiMC component list exceeds its bound",
        ));
    }

    let mut components = BTreeMap::new();
    let mut minecraft = None;
    let mut primary_loader = None;
    for component in dto
        .components
        .iter()
        .filter(|component| !component.disabled)
    {
        if component.uid.is_empty()
            || component.uid.len() > 128
            || component.uid.chars().any(char::is_control)
        {
            return Err(PackError::manifest(
                "mmc-pack.json contains an invalid component UID",
            ));
        }
        let version = component.version.as_deref().ok_or_else(|| {
            PackError::manifest("an active MultiMC component has no exact version")
        })?;
        validate_exact_version(version)?;
        if components
            .insert(component.uid.clone(), version.to_owned())
            .is_some()
        {
            return Err(PackError::manifest(
                "mmc-pack.json repeats an active component UID",
            ));
        }

        match component.uid.as_str() {
            MINECRAFT_UID => {
                if minecraft.replace(version.to_owned()).is_some() {
                    return Err(PackError::manifest(
                        "mmc-pack.json declares more than one Minecraft component",
                    ));
                }
            }
            FABRIC_UID => set_primary_loader(&mut primary_loader, LoaderKind::Fabric, version)?,
            FORGE_UID => {
                set_primary_loader(&mut primary_loader, LoaderKind::Forge, version)?;
            }
            NEOFORGE_UID => {
                set_primary_loader(&mut primary_loader, LoaderKind::NeoForge, version)?;
            }
            uid if AUXILIARY_UIDS.contains(&uid) => {}
            uid if uid == "org.quiltmc.quilt-loader" || uid == "com.mumfrey.liteloader" => {
                return Err(PackError::new(
                    ErrorCode::PackRuntimeUnsupported,
                    "mmc-pack.json declares an unsupported primary loader",
                ));
            }
            _ => {
                return Err(PackError::new(
                    ErrorCode::PackRuntimeUnsupported,
                    "mmc-pack.json declares an unsupported custom component",
                ));
            }
        }
    }

    let minecraft = minecraft.ok_or_else(|| {
        PackError::manifest("mmc-pack.json must declare exactly one Minecraft component")
    })?;
    let runtime = PackRuntimeRequirement::new(minecraft, primary_loader)?;
    Ok((runtime, components))
}

fn set_primary_loader(
    primary_loader: &mut Option<PackLoaderRequirement>,
    kind: LoaderKind,
    version: &str,
) -> Result<(), PackError> {
    let loader = PackLoaderRequirement::new(kind, version.to_owned())?;
    if primary_loader.replace(loader).is_some() {
        return Err(PackError::new(
            ErrorCode::PackRuntimeUnsupported,
            "mmc-pack.json declares more than one supported primary loader",
        ));
    }
    Ok(())
}

fn validate_exact_version(version: &str) -> Result<(), PackError> {
    let lower = version.to_ascii_lowercase();
    let syntax_is_exact = !version.is_empty()
        && version.len() <= 64
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+' | '~'));
    let moving_alias = matches!(
        lower.as_str(),
        "latest" | "recommended" | "stable" | "release" | "snapshot"
    );
    let wildcard_segment = lower
        .split(['.', '-', '_', '+'])
        .any(|segment| matches!(segment, "x" | "latest" | "recommended"));
    if !syntax_is_exact || moving_alias || wildcard_segment {
        return Err(PackError::manifest(
            "MultiMC component version is moving, ambiguous, or malformed",
        ));
    }
    Ok(())
}

fn select_content_root<R: Read + Seek>(
    index: &PackArchiveIndex<R>,
    root_prefix: &str,
) -> Result<String, PackError> {
    let modern = format!("{root_prefix}.minecraft/");
    let historical = format!("{root_prefix}minecraft/");
    let has_modern = has_tree(index, &modern);
    let has_historical = has_tree(index, &historical);
    match (has_modern, has_historical) {
        (true, false) => Ok(modern),
        (false, true) => Ok(historical),
        (true, true) => Err(PackError::manifest(
            "MultiMC export contains both .minecraft and minecraft content roots",
        )),
        (false, false) => Err(PackError::manifest(
            "MultiMC export contains no supported Minecraft content root",
        )),
    }
}

fn has_tree<R: Read + Seek>(index: &PackArchiveIndex<R>, prefix: &str) -> bool {
    index
        .entries()
        .iter()
        .any(|entry| entry.name == prefix || entry.name.starts_with(prefix))
}

fn read_metadata<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    root_prefix: &str,
) -> Result<PackMetadata, PackError> {
    let name = format!("{root_prefix}instance.cfg");
    let display_name = if index.entry(&name).is_some() {
        let bytes = index.read_manifest(&name)?;
        parse_instance_name(&bytes)?
    } else {
        None
    };
    PackMetadata::new(
        display_name.unwrap_or_else(|| "Prism/MultiMC instance".to_owned()),
        None,
        None,
        Vec::new(),
    )
}

fn parse_instance_name(bytes: &[u8]) -> Result<Option<String>, PackError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| PackError::manifest("instance.cfg is not valid UTF-8"))?;
    let mut name = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "name" {
            continue;
        }
        if name.is_some() {
            return Err(PackError::manifest(
                "instance.cfg contains more than one display name",
            ));
        }
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if value.chars().count() > MAX_DISPLAY_STRING_CHARS || value.chars().any(char::is_control) {
            return Err(PackError::manifest(
                "instance.cfg display name exceeds its bound",
            ));
        }
        name = Some(value.to_owned());
    }
    Ok(name)
}

fn reject_jarmods<R: Read + Seek>(
    index: &PackArchiveIndex<R>,
    root_prefix: &str,
) -> Result<(), PackError> {
    if index.entries().iter().any(|entry| {
        entry
            .name
            .strip_prefix(root_prefix)
            .is_some_and(|relative| first_component_is(relative, "jarmods"))
            && !entry.is_directory
    }) {
        return Err(PackError::new(
            ErrorCode::PackRuntimeUnsupported,
            "MultiMC jarmods change the game JAR and cannot be imported",
        ));
    }
    Ok(())
}

fn validate_patches<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    root_prefix: &str,
    components: &BTreeMap<String, String>,
) -> Result<(), PackError> {
    let patch_names: Vec<String> = index
        .entries()
        .iter()
        .filter(|entry| {
            entry.is_regular
                && entry
                    .name
                    .strip_prefix(root_prefix)
                    .is_some_and(|relative| first_component_is(relative, "patches"))
        })
        .map(|entry| entry.name.clone())
        .collect();

    for name in patch_names {
        let bytes = index.read_manifest(&name)?;
        let patch: MultiMcPatchDocument = serde_json::from_slice(&bytes)
            .map_err(|_| PackError::manifest("MultiMC patch JSON is malformed"))?;
        if patch.format_version != Some(1) {
            return Err(PackError::new(
                ErrorCode::PackRuntimeUnsupported,
                "MultiMC patch uses an unsupported format",
            ));
        }
        let Some(component_version) = components.get(&patch.uid) else {
            return Err(PackError::new(
                ErrorCode::PackRuntimeUnsupported,
                "MultiMC patch targets an unsupported foreign component",
            ));
        };
        if component_version != &patch.version {
            return Err(PackError::new(
                ErrorCode::PackRuntimeUnsupported,
                "MultiMC patch changes a component version",
            ));
        }
        for requirement in patch.requires {
            if !requirement.extra.is_empty() {
                return Err(PackError::new(
                    ErrorCode::PackRuntimeUnsupported,
                    "MultiMC patch contains a non-redundant component requirement",
                ));
            }
            let Some(required_version) = components.get(&requirement.uid) else {
                return Err(PackError::new(
                    ErrorCode::PackRuntimeUnsupported,
                    "MultiMC patch adds a non-redundant component requirement",
                ));
            };
            if requirement
                .equals
                .as_ref()
                .is_some_and(|equals| equals != required_version)
            {
                return Err(PackError::new(
                    ErrorCode::PackRuntimeUnsupported,
                    "MultiMC patch changes an exact component requirement",
                ));
            }
            if requirement
                .suggests
                .as_ref()
                .is_some_and(|suggests| suggests != required_version)
            {
                return Err(PackError::new(
                    ErrorCode::PackRuntimeUnsupported,
                    "MultiMC patch suggests a different component version",
                ));
            }
        }
        if patch
            .extra
            .keys()
            .any(|key| key.eq_ignore_ascii_case("jarMods"))
        {
            return Err(PackError::new(
                ErrorCode::PackRuntimeUnsupported,
                "MultiMC patch declares jarmods that cannot be imported",
            ));
        }
        if patch
            .extra
            .keys()
            .any(|key| !REDUNDANT_PATCH_FIELDS.contains(&key.as_str()))
        {
            return Err(PackError::new(
                ErrorCode::PackRuntimeUnsupported,
                "MultiMC patch contains non-redundant launch metadata",
            ));
        }
    }
    Ok(())
}

fn collect_content<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    content_root: &str,
    cancellation: &CancellationToken,
) -> Result<(Vec<EmbeddedMod>, Vec<NormalizedSeedEntry>), PackError> {
    let entries: Vec<(String, bool, bool)> = index
        .entries()
        .iter()
        .filter(|entry| entry.name.starts_with(content_root) && entry.name != content_root)
        .map(|entry| (entry.name.clone(), entry.is_regular, entry.is_directory))
        .collect();
    if entries.len() > MAX_SEED_ENTRIES {
        return Err(PackError::new(
            ErrorCode::PackSourceTooLarge,
            "MultiMC content entry count exceeds its bound",
        ));
    }

    let mut seen = BTreeMap::new();
    let mut embedded_mods = Vec::new();
    let mut seed_entries = Vec::new();
    for (archive_entry, is_regular, is_directory) in entries {
        if is_directory {
            continue;
        }
        let relative = &archive_entry[content_root.len()..];
        let destination = PackPath::normalize(relative)?;
        if first_component_is(destination.as_str(), "jarmods") {
            return Err(PackError::new(
                ErrorCode::PackRuntimeUnsupported,
                "MultiMC jarmods change the game JAR and cannot be imported",
            ));
        }
        if is_foreign_launcher_content(&destination) {
            continue;
        }
        if !is_regular {
            return Err(PackError::archive(
                "MultiMC content contains a special archive entry",
            ));
        }
        let key = destination.collision_key();
        if seen.insert(key, destination.as_str().to_owned()).is_some() {
            return Err(PackError::path(
                "MultiMC content contains colliding destinations",
            ));
        }
        let observed = index.stream_entry_hashed(&archive_entry, &mut sink(), cancellation)?;
        if is_embedded_mod(&destination) {
            embedded_mods.push(EmbeddedMod {
                archive_entry,
                destination,
                size: observed.size,
                sha256: observed.sha256,
            });
        } else {
            seed_entries.push(NormalizedSeedEntry::new(
                archive_entry,
                destination,
                SeedLayer::base(),
                observed.size,
                observed.sha256,
            ));
        }
    }
    Ok((embedded_mods, seed_entries))
}

fn is_embedded_mod(destination: &PackPath) -> bool {
    let components: Vec<&str> = destination.components().collect();
    components.len() == 2
        && components[0].eq_ignore_ascii_case("mods")
        && components[1].to_ascii_lowercase().ends_with(".jar")
}

fn is_foreign_launcher_content(destination: &PackPath) -> bool {
    let path = destination.as_str();
    first_component_is(path, "libraries")
        || first_component_is(path, "accounts")
        || first_component_is(path, "java")
        || first_component_is(path, "settings")
        || destination
            .file_name()
            .eq_ignore_ascii_case("accounts.json")
        || destination
            .file_name()
            .eq_ignore_ascii_case("launcher_accounts.json")
}

fn first_component_is(path: &str, expected: &str) -> bool {
    path.split('/').next().is_some_and(|component| {
        component.eq_ignore_ascii_case(expected)
            && (path.len() == component.len()
                || path.as_bytes().get(component.len()) == Some(&b'/'))
    })
}
