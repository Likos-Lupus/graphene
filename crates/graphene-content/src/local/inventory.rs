use crate::{
    id::{ContentEntryId, ContentFileRef},
    local::{
        fingerprint::{ContentInventoryFingerprint, compute_murmur2, stream_file_hashes},
        metadata::{NormalizedModDescriptor, inspect_mod_archive},
    },
};
use graphene_core::{
    CancellationToken, ErrorCode, ErrorKind, GrapheneError, Result, Sha1Digest, Sha256Digest,
};
use graphene_instance::ManagedRelativePath;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

pub const MAX_MODS_DIR_ENTRIES: usize = 2048;
pub const MAX_ARCHIVE_INSPECT_BYTES: usize = 250 * 1024 * 1024; // 250 MiB limit

/// Status of a local file in the content inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocalFileStatus {
    /// Desired-state managed mod matching lockfile declaration and healthy on disk.
    ManagedHealthy,
    /// Desired-state managed mod with modified / drifted bytes.
    ManagedDrifted,
    /// Unmanaged file recognized by an exact remote provider match.
    RecognizedUnmanaged,
    /// Unmanaged file with recognized local mod metadata (Fabric/Forge/NeoForge/legacy).
    #[default]
    UnmanagedKnownMetadata,
    /// Unmanaged file with valid JAR structure but unknown metadata format.
    UnmanagedUnknown,
    /// Disabled mod file (e.g. .jar.disabled).
    Disabled,
    /// Malformed or corrupted file.
    Invalid,
}

/// A physical content file discovered during local scanning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalContentFile {
    pub relative_path: ManagedRelativePath,
    pub filename: String,
    pub enabled: bool,
    pub size: u64,
    pub sha1: Option<Sha1Digest>,
    pub sha256: Option<Sha256Digest>,
    pub murmur2: Option<u32>,
    pub descriptors: Vec<NormalizedModDescriptor>,
    pub status: LocalFileStatus,
    pub managed_entry_id: Option<ContentEntryId>,
    pub remote_file_ref: Option<ContentFileRef>,
    pub diagnostics: Vec<String>,
}

impl LocalContentFile {
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn primary_mod_id(&self) -> Option<&str> {
        self.descriptors.first().map(|d| d.mod_id.as_str())
    }
}

/// Diagnostic finding for duplicate enabled logical mod IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateModFinding {
    pub mod_id: String,
    pub conflicting_files: Vec<ManagedRelativePath>,
}

/// Complete snapshot of the local mod inventory under an instance's `.minecraft/mods`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalContentInventory {
    pub files: Vec<LocalContentFile>,
    pub duplicate_mod_ids: Vec<DuplicateModFinding>,
    pub fingerprint: ContentInventoryFingerprint,
}

impl LocalContentInventory {
    #[must_use]
    pub fn find_by_path(&self, path: &ManagedRelativePath) -> Option<&LocalContentFile> {
        self.files.iter().find(|f| &f.relative_path == path)
    }

    #[must_use]
    pub fn find_by_entry_id(&self, entry_id: &ContentEntryId) -> Option<&LocalContentFile> {
        self.files
            .iter()
            .find(|f| f.managed_entry_id.as_ref() == Some(entry_id))
    }
}

/// Scans the instance `.minecraft/mods` directory strictly offline.
pub fn scan_local_inventory(
    mods_dir: &Path,
    compute_hashes: bool,
    cancellation: &CancellationToken,
) -> Result<LocalContentInventory> {
    if !mods_dir.exists() {
        return Ok(LocalContentInventory {
            files: Vec::new(),
            duplicate_mod_ids: Vec::new(),
            fingerprint: ContentInventoryFingerprint::compute(std::iter::empty()),
        });
    }

    let read_dir = fs::read_dir(mods_dir).map_err(|source| {
        GrapheneError::new(
            ErrorCode::FileOpenFailed,
            ErrorKind::Filesystem,
            format!("failed to read mods directory: {}", mods_dir.display()),
        )
        .with_source(source)
    })?;

    let mut dir_entries: Vec<PathBuf> = Vec::new();
    for entry_res in read_dir {
        if cancellation.is_cancelled() {
            return Err(GrapheneError::new(
                ErrorCode::OperationCancelled,
                ErrorKind::Cancelled,
                "local mod scan was cancelled",
            ));
        }
        let entry = entry_res.map_err(|source| {
            GrapheneError::new(
                ErrorCode::FileOpenFailed,
                ErrorKind::Filesystem,
                "failed reading directory entry in mods folder",
            )
            .with_source(source)
        })?;

        let file_type = entry.file_type().map_err(|source| {
            GrapheneError::new(
                ErrorCode::FileOpenFailed,
                ErrorKind::Filesystem,
                "failed reading file type in mods folder",
            )
            .with_source(source)
        })?;

        // Strictly do not follow symlinks or recurse into subdirectories
        if file_type.is_symlink() || file_type.is_dir() {
            continue;
        }

        dir_entries.push(entry.path());
        if dir_entries.len() > MAX_MODS_DIR_ENTRIES {
            return Err(GrapheneError::new(
                ErrorCode::StorageLayoutInvalid,
                ErrorKind::Storage,
                format!("mods directory exceeds maximum entry count of {MAX_MODS_DIR_ENTRIES}"),
            ));
        }
    }

    // Deterministic alphabetical sorting
    dir_entries.sort();

    let mut scanned_files = Vec::new();
    let mut mod_id_locations: HashMap<String, Vec<ManagedRelativePath>> = HashMap::new();

    for path in dir_entries {
        if cancellation.is_cancelled() {
            return Err(GrapheneError::new(
                ErrorCode::OperationCancelled,
                ErrorKind::Cancelled,
                "local mod scan was cancelled",
            ));
        }

        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        let (enabled, is_mod_candidate) = if file_name.ends_with(".jar") {
            (true, true)
        } else if file_name.ends_with(".jar.disabled") {
            (false, true)
        } else {
            (false, false)
        };

        if !is_mod_candidate {
            continue;
        }

        let rel_path_str = format!(".minecraft/mods/{file_name}");
        let relative_path = match ManagedRelativePath::new(&rel_path_str) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(err) => {
                scanned_files.push(LocalContentFile {
                    relative_path,
                    filename: file_name,
                    enabled,
                    size: 0,
                    sha1: None,
                    sha256: None,
                    murmur2: None,
                    descriptors: Vec::new(),
                    status: LocalFileStatus::Invalid,
                    managed_entry_id: None,
                    remote_file_ref: None,
                    diagnostics: vec![format!("failed reading file metadata: {err}")],
                });
                continue;
            }
        };

        let size = metadata.len();
        let mut diagnostics = Vec::new();

        let (sha1, sha256, murmur2) = if compute_hashes {
            match stream_file_hashes(&path, cancellation) {
                Ok((h1, h256, _)) => {
                    let m2 = match fs::read(&path) {
                        Ok(bytes) => Some(compute_murmur2(&bytes)),
                        Err(_) => None,
                    };
                    (Some(h1), Some(h256), m2)
                }
                Err(err) => {
                    diagnostics.push(format!("failed computing hashes: {}", err.message()));
                    (None, None, None)
                }
            }
        } else {
            (None, None, None)
        };

        let descriptors = if size as usize <= MAX_ARCHIVE_INSPECT_BYTES {
            match fs::read(&path) {
                Ok(bytes) => match inspect_mod_archive(&bytes, cancellation) {
                    Ok(descs) => descs,
                    Err(err) => {
                        diagnostics.push(format!(
                            "failed inspecting archive metadata: {}",
                            err.message()
                        ));
                        Vec::new()
                    }
                },
                Err(err) => {
                    diagnostics.push(format!("failed reading file bytes: {err}"));
                    Vec::new()
                }
            }
        } else {
            diagnostics.push("file size exceeds maximum inspectable archive size".to_string());
            Vec::new()
        };

        let status = if !diagnostics.is_empty() && descriptors.is_empty() {
            LocalFileStatus::Invalid
        } else if !enabled {
            LocalFileStatus::Disabled
        } else if !descriptors.is_empty() {
            LocalFileStatus::UnmanagedKnownMetadata
        } else {
            LocalFileStatus::UnmanagedUnknown
        };

        if enabled {
            for desc in &descriptors {
                mod_id_locations
                    .entry(desc.mod_id.clone())
                    .or_default()
                    .push(relative_path.clone());
            }
        }

        scanned_files.push(LocalContentFile {
            relative_path,
            filename: file_name,
            enabled,
            size,
            sha1,
            sha256,
            murmur2,
            descriptors,
            status,
            managed_entry_id: None,
            remote_file_ref: None,
            diagnostics,
        });
    }

    let mut duplicate_mod_ids = Vec::new();
    for (mod_id, paths) in mod_id_locations {
        if paths.len() > 1 {
            duplicate_mod_ids.push(DuplicateModFinding {
                mod_id,
                conflicting_files: paths,
            });
        }
    }
    duplicate_mod_ids.sort_by(|a, b| a.mod_id.cmp(&b.mod_id));

    let fingerprint = ContentInventoryFingerprint::compute(
        scanned_files
            .iter()
            .map(|f| (&f.relative_path, f.enabled, f.size, f.sha256.as_ref())),
    );

    Ok(LocalContentInventory {
        files: scanned_files,
        duplicate_mod_ids,
        fingerprint,
    })
}
