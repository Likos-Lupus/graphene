//! Explicit generic archive import support (no magic runtime inference).
//!
//! Generic mode never participates in content-based detection. The caller supplies exact runtime
//! requirements and chooses how the archive tree maps into the instance `.minecraft` directory.

use crate::archive::limits::{MAX_MANAGED_FILES, MAX_SEED_ENTRIES};
use crate::archive::{PackArchiveIndex, PackPath};
use crate::error::PackError;
use crate::format::detect_pack_format;
use crate::model::{
    NormalizedSeedEntry, PackFormat, PackLoaderRequirement, PackRuntimeRequirement, SeedLayer,
};
use graphene_core::{CancellationToken, ErrorCode, Sha256Digest};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, sink};

/// The archive directory whose contents become relative paths under `.minecraft`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenericContentRoot {
    /// Select the archive root only when its interpretation is unambiguous.
    Auto,
    /// Keep every archive path relative to the archive root.
    ArchiveRoot,
    /// Strip this exact archive directory prefix before mapping payload paths.
    Directory(String),
}

/// Caller-owned runtime and archive-layout choices required by generic import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericImportOptions {
    runtime: PackRuntimeRequirement,
    content_root: GenericContentRoot,
}

impl GenericImportOptions {
    /// Creates options from an exact Minecraft version and an optional exact supported loader.
    pub fn new(
        minecraft_version: impl Into<String>,
        primary_loader: Option<PackLoaderRequirement>,
        content_root: GenericContentRoot,
    ) -> Result<Self, PackError> {
        let minecraft_version = minecraft_version.into();
        require_exact_version("Minecraft", &minecraft_version)?;
        if let Some(loader) = &primary_loader {
            require_exact_version("loader", loader.version())?;
        }
        let runtime = PackRuntimeRequirement::new(minecraft_version, primary_loader)?;
        validate_content_root(&content_root)?;
        Ok(Self {
            runtime,
            content_root,
        })
    }

    #[must_use]
    pub const fn runtime(&self) -> &PackRuntimeRequirement {
        &self.runtime
    }

    #[must_use]
    pub const fn content_root(&self) -> &GenericContentRoot {
        &self.content_root
    }
}

/// One embedded `mods/*.jar` payload eligible for later cache ingestion as managed content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericEmbeddedMod {
    archive_entry: String,
    destination: PackPath,
    size: u64,
    sha256: Sha256Digest,
}

impl GenericEmbeddedMod {
    #[must_use]
    pub fn archive_entry(&self) -> &str {
        &self.archive_entry
    }

    /// Destination relative to the instance `.minecraft` directory.
    #[must_use]
    pub const fn destination(&self) -> &PackPath {
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

/// Fully validated, deterministic generic archive payload classification.
#[derive(Debug, Clone)]
pub struct ParsedGenericPack {
    runtime: PackRuntimeRequirement,
    content_root_prefix: String,
    embedded_mods: Vec<GenericEmbeddedMod>,
    seed_entries: Vec<NormalizedSeedEntry>,
}

impl ParsedGenericPack {
    #[must_use]
    pub const fn runtime(&self) -> &PackRuntimeRequirement {
        &self.runtime
    }

    /// Effective archive prefix, including a trailing slash when a directory root was selected.
    #[must_use]
    pub fn content_root_prefix(&self) -> &str {
        &self.content_root_prefix
    }

    #[must_use]
    pub fn embedded_mods(&self) -> &[GenericEmbeddedMod] {
        &self.embedded_mods
    }

    /// Ordinary payload files, with destinations relative to `.minecraft`.
    #[must_use]
    pub fn seed_entries(&self) -> &[NormalizedSeedEntry] {
        &self.seed_entries
    }
}

/// Normalizes an explicitly selected generic archive without executing or inspecting payloads.
pub fn normalize<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    options: &GenericImportOptions,
) -> Result<ParsedGenericPack, PackError> {
    normalize_with_cancellation(index, options, &CancellationToken::new())
}

/// Cancellation-aware form of [`normalize`].
pub fn normalize_with_cancellation<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    options: &GenericImportOptions,
    cancellation: &CancellationToken,
) -> Result<ParsedGenericPack, PackError> {
    reject_known_format(index)?;
    let prefix = select_content_root(index, options.content_root())?;
    let (selected, directories) = collect_selected_entries(index, &prefix)?;
    validate_destination_map(&selected, &directories)?;

    let mod_count = selected
        .iter()
        .filter(|entry| is_embedded_mod(&entry.destination))
        .count();
    if mod_count > MAX_MANAGED_FILES || selected.len().saturating_sub(mod_count) > MAX_SEED_ENTRIES
    {
        return Err(PackError::new(
            ErrorCode::PackSourceTooLarge,
            "generic payload file count exceeds its bound",
        ));
    }

    let mut embedded_mods = Vec::with_capacity(mod_count);
    let mut seed_entries = Vec::with_capacity(selected.len().saturating_sub(mod_count));
    for entry in selected {
        let observed =
            index.stream_entry_hashed(&entry.archive_entry, &mut sink(), cancellation)?;
        if is_embedded_mod(&entry.destination) {
            embedded_mods.push(GenericEmbeddedMod {
                archive_entry: entry.archive_entry,
                destination: entry.destination,
                size: observed.size,
                sha256: observed.sha256,
            });
        } else {
            seed_entries.push(NormalizedSeedEntry::new(
                entry.archive_entry,
                entry.destination,
                SeedLayer::base(),
                observed.size,
                observed.sha256,
            ));
        }
    }

    embedded_mods.sort_by(|a, b| a.destination.as_str().cmp(b.destination.as_str()));
    seed_entries.sort_by(|a, b| a.destination().as_str().cmp(b.destination().as_str()));
    Ok(ParsedGenericPack {
        runtime: options.runtime.clone(),
        content_root_prefix: prefix,
        embedded_mods,
        seed_entries,
    })
}

#[derive(Debug)]
struct SelectedEntry {
    archive_entry: String,
    destination: PackPath,
}

fn reject_known_format<R: Read + Seek>(index: &mut PackArchiveIndex<R>) -> Result<(), PackError> {
    match detect_pack_format(index, false) {
        Ok(detection) => return recognized_format_error(detection.format),
        Err(error) if error.code() == ErrorCode::PackFormatUnknown => {}
        Err(error) => return Err(error),
    }

    let mut candidate_roots = BTreeSet::new();
    for entry in index.entries() {
        for manifest in [
            "graphene.pack.json",
            "modrinth.index.json",
            "manifest.json",
            "mmc-pack.json",
        ] {
            if entry.name == manifest {
                candidate_roots.insert(String::new());
            } else if let Some(prefix) = entry.name.strip_suffix(manifest)
                && prefix.ends_with('/')
            {
                candidate_roots.insert(prefix.to_owned());
            }
        }
    }

    for root in candidate_roots {
        let recognized = recognized_formats_at_root(index, &root)?;
        if recognized.len() > 1 {
            return Err(PackError::new(
                ErrorCode::PackFormatAmbiguous,
                "archive declares multiple pack formats",
            ));
        }
        if let Some(format) = recognized.into_iter().next() {
            return recognized_format_error(format);
        }
    }
    Ok(())
}

fn recognized_formats_at_root<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    root: &str,
) -> Result<Vec<PackFormat>, PackError> {
    let mut formats = Vec::new();
    let graphene_name = format!("{root}graphene.pack.json");
    if index.entry(&graphene_name).is_some()
        && super::graphene::recognizes_manifest(index.read_manifest(&graphene_name)?.as_slice())?
    {
        formats.push(PackFormat::Graphene);
    }
    if super::modrinth::recognizes(index, root)? {
        formats.push(PackFormat::Modrinth);
    }
    if super::curseforge::recognizes(index, root)? {
        formats.push(PackFormat::CurseForge);
    }
    if super::multimc::recognizes(index, root)? {
        formats.push(PackFormat::PrismMultiMc);
    }
    Ok(formats)
}

fn recognized_format_error<T>(format: PackFormat) -> Result<T, PackError> {
    Err(PackError::new(
        ErrorCode::PackSourceInvalid,
        format!("generic import cannot bypass the recognized {format:?} pack format"),
    ))
}

fn select_content_root<R: Read + Seek>(
    index: &PackArchiveIndex<R>,
    root: &GenericContentRoot,
) -> Result<String, PackError> {
    match root {
        GenericContentRoot::ArchiveRoot => Ok(String::new()),
        GenericContentRoot::Directory(directory) => {
            let prefix = format!("{directory}/");
            let has_payload = index.entries().iter().any(|entry| {
                !entry.is_directory
                    && entry.name.starts_with(&prefix)
                    && entry.name.len() > prefix.len()
            });
            if !has_payload {
                return Err(PackError::new(
                    ErrorCode::PackSelectionRequired,
                    "selected generic content root contains no payload files",
                ));
            }
            Ok(prefix)
        }
        GenericContentRoot::Auto => select_automatic_root(index),
    }
}

fn select_automatic_root<R: Read + Seek>(index: &PackArchiveIndex<R>) -> Result<String, PackError> {
    let names: Vec<&str> = index
        .entries()
        .iter()
        .filter(|entry| !entry.is_directory)
        .map(|entry| entry.name.as_str())
        .collect();
    if names.is_empty() {
        return Err(PackError::invalid("generic archive has no payload files"));
    }
    if crate::archive::detect_wrapper_root(&names).is_some() {
        return Err(PackError::new(
            ErrorCode::PackSelectionRequired,
            "generic archive has a wrapper/root ambiguity; choose the content root explicitly",
        ));
    }
    Ok(String::new())
}

fn collect_selected_entries<R: Read + Seek>(
    index: &PackArchiveIndex<R>,
    prefix: &str,
) -> Result<(Vec<SelectedEntry>, Vec<PackPath>), PackError> {
    let mut selected = Vec::new();
    let mut directories = Vec::new();
    for entry in index.entries() {
        if !entry.name.starts_with(prefix) || entry.name.len() <= prefix.len() {
            continue;
        }
        let relative = &entry.name[prefix.len()..];
        if entry.is_directory {
            if let Some(directory) = normalize_directory_path(relative)? {
                directories.push(directory);
            }
            continue;
        }
        if !entry.is_regular || entry.is_symlink {
            return Err(PackError::archive(
                "generic content root contains a special archive entry",
            ));
        }
        let destination = PackPath::normalize(relative)?;
        reject_reserved_destination(&destination)?;
        selected.push(SelectedEntry {
            archive_entry: entry.name.clone(),
            destination,
        });
    }
    if selected.is_empty() {
        return Err(PackError::invalid(
            "selected generic content root has no ordinary files",
        ));
    }
    Ok((selected, directories))
}

fn normalize_directory_path(relative: &str) -> Result<Option<PackPath>, PackError> {
    let directory = relative.strip_suffix('/').unwrap_or(relative);
    if directory.is_empty() {
        return Ok(None);
    }
    let path = PackPath::normalize(directory)?;
    reject_reserved_destination(&path)?;
    Ok(Some(path))
}

fn validate_destination_map(
    entries: &[SelectedEntry],
    directories: &[PackPath],
) -> Result<(), PackError> {
    let mut paths = BTreeMap::<String, (&str, bool)>::new();
    for directory in directories {
        insert_destination_nodes(&mut paths, directory, false)?;
    }
    for entry in entries {
        insert_destination_nodes(&mut paths, &entry.destination, true)?;
    }
    Ok(())
}

fn insert_destination_nodes<'a>(
    paths: &mut BTreeMap<String, (&'a str, bool)>,
    path: &'a PackPath,
    final_is_file: bool,
) -> Result<(), PackError> {
    let raw = path.as_str();
    let component_count = path.components().count();
    for (index, end) in raw
        .match_indices('/')
        .map(|(index, _)| index)
        .chain(std::iter::once(raw.len()))
        .enumerate()
    {
        let display = &raw[..end];
        let is_file = final_is_file && index + 1 == component_count;
        let key = display.to_lowercase();
        if let Some((existing, existing_is_file)) = paths.get(&key) {
            if *existing != display {
                return Err(PackError::path(format!(
                    "generic destinations collide case-insensitively: {existing}"
                )));
            }
            if *existing_is_file || is_file {
                let message = if *existing_is_file == is_file {
                    "generic payload repeats a destination"
                } else {
                    "generic destinations have a file/directory prefix collision"
                };
                return Err(PackError::path(message));
            }
            continue;
        }
        paths.insert(key, (display, is_file));
    }
    Ok(())
}

fn reject_reserved_destination(destination: &PackPath) -> Result<(), PackError> {
    let first = destination
        .components()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let reserved_root = matches!(
        first.as_str(),
        ".graphene" | "assets" | "libraries" | "natives" | "runtime" | "versions"
    );
    let reserved_file = destination.parent().is_none()
        && matches!(
            first.as_str(),
            "instance.cfg"
                | "instance.json"
                | "launcher_accounts.json"
                | "launcher_profiles.json"
                | "launcher_settings.json"
                | "mmc-pack.json"
                | "prismlauncher.cfg"
        );
    if reserved_root || reserved_file {
        return Err(PackError::path(
            "generic payload targets a reserved runtime or management path",
        ));
    }
    Ok(())
}

fn is_embedded_mod(destination: &PackPath) -> bool {
    let mut components = destination.components();
    matches!(components.next(), Some("mods"))
        && components
            .next()
            .is_some_and(|name| name.to_ascii_lowercase().ends_with(".jar"))
        && components.next().is_none()
}

fn validate_content_root(root: &GenericContentRoot) -> Result<(), PackError> {
    let GenericContentRoot::Directory(directory) = root else {
        return Ok(());
    };
    if directory.ends_with('/') {
        return Err(PackError::path(
            "generic content root must not have a trailing slash",
        ));
    }
    PackPath::normalize(directory).map(|_| ())
}

fn require_exact_version(label: &str, version: &str) -> Result<(), PackError> {
    let moving = version
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|token| {
            matches!(
                token.to_ascii_lowercase().as_str(),
                "latest" | "recommended" | "stable" | "release" | "snapshot" | "x"
            )
        });
    if moving || version.contains('*') {
        return Err(PackError::new(
            ErrorCode::PackRuntimeUnsupported,
            format!("generic import requires an exact {label} version"),
        ));
    }
    Ok(())
}
