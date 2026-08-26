use crate::context::ServiceContext;
use graphene_core::{
    ErrorCode, ErrorKind, GrapheneError, InstanceId, OperationController, Result, Sha256Digest,
};
use graphene_instance::{InstalledComponent, InstalledComponentKind, InstanceLockfile};
use graphene_modpack::{ManagedSource, PackMetadata, PackPath};
use serde::Serialize;
use sha2::Digest as _;
use std::{
    collections::BTreeSet,
    io::Read as _,
    path::{Path, PathBuf},
    sync::Arc,
};

pub(crate) const EXPORT_PLAN_SCHEMA_VERSION: u32 = 1;
const MAX_EXPORT_SEED_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_LOCKFILE_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MANIFEST_ENTRY_NAME: &str = "graphene.pack.json";

/// How managed binaries without persistable sources are handled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportEmbeddingPolicy {
    /// Reference-only export; source-less managed artifacts are excluded with diagnostics.
    ReferenceOnly,
    /// Embed only caller-approved managed artifacts identified by their `.minecraft/`-relative
    /// destination. Embedding is a distribution decision owned by the host.
    EmbedExplicit(BTreeSet<String>),
}

/// Plan-before-execute request describing a conservative Graphene pack export.
#[derive(Debug, Clone)]
pub struct ModpackExportRequest {
    instance_id: InstanceId,
    metadata: PackMetadata,
    output: PathBuf,
    seed_selection: Vec<String>,
    embedding: ExportEmbeddingPolicy,
}

impl ModpackExportRequest {
    pub fn new(
        instance_id: InstanceId,
        name: impl Into<String>,
        version: Option<String>,
        summary: Option<String>,
        output: PathBuf,
        embedding: ExportEmbeddingPolicy,
    ) -> Result<Self> {
        if output.as_os_str().is_empty() {
            return Err(modpack_error(
                ErrorCode::PackExportInvalid,
                "export destination path is empty",
            ));
        }
        let metadata = PackMetadata::new(name, version, summary, Vec::new()).map_err(|error| {
            modpack_error(
                ErrorCode::PackExportInvalid,
                format!("export pack metadata is invalid: {error}"),
            )
        })?;
        Ok(Self {
            instance_id,
            metadata,
            output,
            seed_selection: Vec::new(),
            embedding,
        })
    }

    /// Selects one user-mutable file relative to the instance `.minecraft/` directory as seed
    /// payload.
    ///
    /// # Errors
    /// Fails when the relative path is malformed or reserved launcher state.
    pub fn with_seed(mut self, relative: impl Into<String>) -> Result<Self> {
        let relative = relative.into();
        validate_seed_relative(&relative)?;
        self.seed_selection.push(relative);
        Ok(self)
    }

    #[must_use]
    pub const fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }

    #[must_use]
    pub fn output(&self) -> &Path {
        &self.output
    }
}

/// One planned managed artifact entry.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct PlannedManagedFile {
    pub destination: String,
    pub logical_key: String,
    pub sources: Vec<String>,
    pub size: Option<u64>,
    pub declared_sha1: Option<String>,
    pub declared_sha512: Option<String>,
    pub embed: bool,
    pub(crate) cache_integrity: Option<graphene_core::ArtifactIntegrity>,
    pub provider: Option<String>,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub file_id: Option<String>,
}

/// One planned seed payload entry snapshotted at planning time.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct PlannedSeedFile {
    pub relative: String,
    pub size: u64,
    pub sha256: String,
}

/// Deterministic export plan produced by [`crate::ModpackService::plan_export`].
#[derive(Debug, Clone)]
pub struct ModpackExportPlan {
    pub(crate) schema_version: u32,
    pub(crate) instance_id: InstanceId,
    pub(crate) output: PathBuf,
    pub(crate) metadata: PackMetadata,
    pub(crate) minecraft_version: String,
    pub(crate) primary_loader: Option<(String, String)>,
    pub(crate) managed: Vec<PlannedManagedFile>,
    pub(crate) seeds: Vec<PlannedSeedFile>,
    pub(crate) diagnostics: Vec<(String, String)>,
    pub(crate) fingerprint: String,
}

impl ModpackExportPlan {
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    #[must_use]
    pub const fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }

    #[must_use]
    pub fn output(&self) -> &Path {
        &self.output
    }

    #[must_use]
    pub fn managed_count(&self) -> usize {
        self.managed.len()
    }

    #[must_use]
    pub fn embedded_count(&self) -> usize {
        self.managed.iter().filter(|file| file.embed).count()
    }

    #[must_use]
    pub fn seed_count(&self) -> usize {
        self.seeds.len()
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[(String, String)] {
        &self.diagnostics
    }

    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

/// Successful export outcome.
#[derive(Debug, Clone)]
pub struct ModpackExportResult {
    pub output: PathBuf,
    pub archive_sha256: String,
    pub archive_size: u64,
    pub referenced_managed_files: usize,
    pub embedded_objects: usize,
    pub seed_files: usize,
}

pub(crate) fn modpack_error(code: ErrorCode, message: impl Into<String>) -> GrapheneError {
    GrapheneError::new(code, ErrorKind::Modpack, message)
}

fn validate_seed_relative(relative: &str) -> Result<()> {
    // Structural validation through the pack path model plus explicit reservation of private
    // launcher state that must never travel inside an exported pack.
    if relative.is_empty() || relative.len() > 1024 {
        return Err(modpack_error(
            ErrorCode::PackExportInvalid,
            "seed selection is empty or unbounded",
        ));
    }
    PackPath::normalize(relative).map_err(|error| {
        modpack_error(
            ErrorCode::PackExportInvalid,
            format!("seed selection is not a safe relative path: {error}"),
        )
    })?;
    reject_reserved_selection(relative)
}

fn reject_reserved_selection(relative: &str) -> Result<()> {
    let lowered = relative.to_ascii_lowercase();
    if lowered == "instance.json"
        || lowered.starts_with(".graphene/")
        || lowered == ".graphene"
        || lowered.split('/').any(|part| part == "..")
    {
        return Err(modpack_error(
            ErrorCode::PackExportInvalid,
            "seed selection must never include private launcher state",
        ));
    }
    Ok(())
}

/// Plans a conservative Graphene pack export from committed instance desired state.
pub(crate) async fn plan_export(
    context: &Arc<ServiceContext>,
    request: &ModpackExportRequest,
    operation: &OperationController,
) -> Result<ModpackExportPlan> {
    operation.set_stage("plan-export")?;
    let repo = crate::InstanceRepository::new(context.storage.path());
    let _committed = repo.load_committed(*request.instance_id())?;
    let lockfile = read_lockfile(context, *request.instance_id())?;

    let primary_loader = loader_from_components(&lockfile.components)?;
    let minecraft_dir = repo.paths().minecraft_dir(*request.instance_id());

    let mut managed = Vec::new();
    let mut diagnostics = Vec::new();
    let embed_set = match &request.embedding {
        ExportEmbeddingPolicy::EmbedExplicit(set) => Some(set.clone()),
        ExportEmbeddingPolicy::ReferenceOnly => None,
    };

    for entry in &lockfile.content {
        if !entry.enabled {
            continue;
        }
        let Some(artifact) = lockfile
            .artifacts
            .iter()
            .find(|candidate| candidate.logical_key == entry.artifact_logical_key)
        else {
            continue;
        };
        let Some(destination) = artifact.destination.as_str().strip_prefix(".minecraft/") else {
            continue;
        };

        // Persisted sources are re-checked against the structural URL policy; anything that
        // cannot satisfy it is treated as source-less rather than exported blindly.
        let sources: Vec<String> = artifact
            .sources
            .iter()
            .filter_map(|source| {
                ManagedSource::parse(source.url(), |_| true)
                    .ok()
                    .map(|parsed| parsed.as_str().to_owned())
            })
            .collect();

        let embed = embed_set
            .as_ref()
            .is_some_and(|set| set.contains(destination));
        if sources.is_empty() && !embed {
            diagnostics.push((
                "EXPORT_EMBEDDING_DECISION_REQUIRED".to_owned(),
                format!("managed artifact '{destination}' has no persistable source"),
            ));
            continue;
        }

        managed.push(PlannedManagedFile {
            destination: destination.to_owned(),
            logical_key: artifact.logical_key.clone(),
            sources,
            size: artifact.expected_size,
            declared_sha1: artifact.integrity.sha1().map(|digest| digest.to_string()),
            declared_sha512: artifact.integrity.sha512().map(|digest| digest.to_string()),
            embed,
            cache_integrity: Some(artifact.integrity.clone()),
            provider: entry.provider.clone(),
            project_id: entry.project_id.clone(),
            version_id: entry.version_id.clone(),
            file_id: entry.file_id.clone(),
        });
    }
    managed.sort_by(|left, right| left.destination.cmp(&right.destination));

    operation.set_stage("snapshot-export-files")?;
    let mut seeds = Vec::with_capacity(request.seed_selection.len());
    for relative in &request.seed_selection {
        let absolute = minecraft_dir.join(relative);
        let (size, sha256) = hash_file_bounded(&absolute, MAX_EXPORT_SEED_FILE_BYTES).await?;
        seeds.push(PlannedSeedFile {
            relative: relative.clone(),
            size,
            sha256: sha256.to_string(),
        });
    }
    seeds.sort_by(|left, right| left.relative.cmp(&right.relative));

    let fingerprint = fingerprint_plan(
        request.instance_id(),
        &lockfile.minecraft_version,
        &lockfile.components,
        &managed,
        &seeds,
    );

    Ok(ModpackExportPlan {
        schema_version: EXPORT_PLAN_SCHEMA_VERSION,
        instance_id: *request.instance_id(),
        output: request.output.clone(),
        metadata: request.metadata.clone(),
        minecraft_version: lockfile.minecraft_version.clone(),
        primary_loader,
        managed,
        seeds,
        diagnostics,
        fingerprint,
    })
}

pub(crate) fn read_lockfile(
    context: &Arc<ServiceContext>,
    instance_id: InstanceId,
) -> Result<InstanceLockfile> {
    let path = crate::InstanceRepository::new(context.storage.path())
        .paths()
        .lockfile_path(instance_id);
    let bytes = std::fs::read(&path).map_err(|source| {
        modpack_error(
            ErrorCode::PackPlanStale,
            "instance lockfile is missing; only committed content instances can be exported",
        )
        .with_source(source)
    })?;
    if bytes.len() > MAX_LOCKFILE_BYTES {
        return Err(modpack_error(
            ErrorCode::PackSourceTooLarge,
            "instance lockfile exceeds its read bound",
        ));
    }
    let lockfile: InstanceLockfile = serde_json::from_slice(&bytes).map_err(|source| {
        modpack_error(
            ErrorCode::InstanceLockfileInvalid,
            "instance lockfile is malformed",
        )
        .with_source(source)
    })?;
    lockfile.validate().map_err(|error| {
        modpack_error(
            ErrorCode::InstanceLockfileInvalid,
            "instance lockfile failed validation",
        )
        .with_source(error)
    })?;
    Ok(lockfile)
}

fn loader_from_components(components: &[InstalledComponent]) -> Result<Option<(String, String)>> {
    let loaders: Vec<&InstalledComponent> = components
        .iter()
        .filter(|component| component.kind == InstalledComponentKind::Loader)
        .collect();
    match loaders.as_slice() {
        [] => Ok(None),
        [loader] => {
            let kind = match loader.provider.to_ascii_lowercase().as_str() {
                "fabric" => "fabric",
                "forge" => "forge",
                "neoforge" => "neoforge",
                other => {
                    return Err(modpack_error(
                        ErrorCode::PackRuntimeUnsupported,
                        format!("installed loader '{other}' has no portable pack identity"),
                    ));
                }
            };
            Ok(Some((kind.to_owned(), loader.version.clone())))
        }
        [..] => Err(modpack_error(
            ErrorCode::PackRuntimeUnsupported,
            "instance declares more than one primary loader",
        )),
    }
}

pub(crate) async fn hash_file_bounded(path: &Path, max_bytes: u64) -> Result<(u64, Sha256Digest)> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let metadata = std::fs::symlink_metadata(&path).map_err(|source| {
            modpack_error(
                ErrorCode::PackPlanStale,
                "selected seed file disappeared before export planning",
            )
            .with_source(source)
        })?;
        if !metadata.is_file() {
            return Err(modpack_error(
                ErrorCode::PackExportInvalid,
                "seed selection must be an ordinary file",
            ));
        }
        let mut file = std::fs::File::open(&path).map_err(|source| {
            modpack_error(ErrorCode::PackExportStale, "seed file is unreadable").with_source(source)
        })?;
        let mut hasher = sha2::Sha256::new();
        let mut remaining = max_bytes;
        let mut buffer = [0_u8; 64 * 1024];
        let mut total = 0_u64;
        loop {
            let read = file.read(&mut buffer).map_err(|source| {
                modpack_error(ErrorCode::PackExportInvalid, "seed file read failed")
                    .with_source(source)
            })?;
            if read == 0 {
                break;
            }
            total += read as u64;
            if total > max_bytes {
                return Err(modpack_error(
                    ErrorCode::PackSourceTooLarge,
                    "seed file exceeds the per-file export bound",
                ));
            }
            remaining = remaining.saturating_sub(read as u64);
            hasher.update(&buffer[..read]);
            let _ = remaining;
        }
        let digest_bytes: [u8; 32] = hasher.finalize().into();
        Ok((total, Sha256Digest::from_bytes(digest_bytes)))
    })
    .await
    .map_err(|source| {
        GrapheneError::new(
            ErrorCode::InternalInvariantViolation,
            ErrorKind::Internal,
            "blocking seed hashing worker failed",
        )
        .with_source(source)
    })?
}

pub(crate) fn fingerprint_plan(
    instance_id: &InstanceId,
    minecraft_version: &str,
    components: &[InstalledComponent],
    managed: &[PlannedManagedFile],
    seeds: &[PlannedSeedFile],
) -> String {
    #[derive(Serialize)]
    struct ComponentRef<'a> {
        uid: &'a str,
        version: &'a str,
        provider: &'a str,
    }
    #[derive(Serialize)]
    struct FingerprintInput<'a> {
        instance_id: String,
        minecraft_version: &'a str,
        components: Vec<ComponentRef<'a>>,
        managed: &'a [PlannedManagedFile],
        seeds: &'a [PlannedSeedFile],
    }
    let input = FingerprintInput {
        instance_id: instance_id.to_string(),
        minecraft_version,
        components: components
            .iter()
            .map(|component| ComponentRef {
                uid: component.uid.as_str(),
                version: component.version.as_str(),
                provider: component.provider.as_str(),
            })
            .collect(),
        managed,
        seeds,
    };
    let canonical = serde_json::to_vec(&input).unwrap_or_default();
    let digest = sha2::Sha256::digest(&canonical);
    let bytes: [u8; 32] = digest.into();
    Sha256Digest::from_bytes(bytes).to_string()
}
