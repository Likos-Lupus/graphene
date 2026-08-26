use super::{
    resolution::{ResolvedPackFile, resolve_pending_provider_files},
    source::{artifact_id_for_digest, open_cached_snapshot_path, snapshot_artifact},
};
use crate::{
    adapters::ServiceMetadataAcquirer, component_install::plan_component_install,
    context::ServiceContext, install_service::current_rule_context,
};
use graphene_content::{ContentDependency, DependencyTarget};
use graphene_core::{
    Artifact, ArtifactId, ArtifactIntegrity, ArtifactKind, ArtifactSource, CachePolicy, ErrorCode,
    ErrorKind, GrapheneError, InstanceId, OperationController, Progress, Result, Sha256Digest,
};
use graphene_install::{
    ComponentInstallRequest, InstallPlan, InstallRequest, Materialization, MaterializationScope,
    PlannedArtifact, SeedArchiveLayer,
};
use graphene_instance::{
    LockedContentDependency, LockedContentEntry, LockedPackOrigin, ManagedRelativePath,
    NewInstanceSpec,
};
use graphene_minecraft::{
    LoaderSelection, LoaderVersion, LoaderVersionSelector, ManagedPath, MinecraftVersionId,
};
use graphene_modpack::{
    EmbeddedPackFile, FileSelection, NormalizedModpack, OptionalSelectionPolicy, PackArchiveIndex,
    PackFormat, PackSourceSnapshot, ProviderFileRef, archive::limits::MAX_DISPLAY_STRING_CHARS,
    model::modpack::is_mod_destination,
};
use graphene_providers::MojangProvider;
use serde::Serialize;
use sha2::Digest;
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

/// Current composite import plan contract.
pub const MODPACK_IMPORT_PLAN_VERSION: u32 = 1;

const MANAGED_DESTINATION_PREFIX: &str = ".minecraft/";
/// Logical-key namespace shared with the installer lockfile assembly (`instance:<destination>`).
const CONTENT_KEY_PREFIX: &str = "instance:";
const MODS_DIRECTORY: &str = "mods";

/// Host-owned import request. Everything moving (provider catalogs, loader metadata) is resolved
/// before the plan returns; execution only acquires exact planned artifacts.
#[derive(Debug, Clone)]
pub struct ModpackImportRequest {
    /// Immutable identity pinned by inspection; execution never refetches any URL.
    pub snapshot: PackSourceSnapshot,
    pub target: NewInstanceSpec,
    pub optional_policy: OptionalSelectionPolicy,
}

/// Deterministic composite plan wrapping the ordinary install transaction payload.
#[derive(Debug, Clone)]
pub struct ModpackImportPlan {
    pub schema_version: u32,
    pub install_plan: InstallPlan,
    pub pack_format: PackFormat,
    pub origin: LockedPackOrigin,
    pub fingerprint: Sha256Digest,
}

/// Builds a complete create-only pack import plan from the pinned snapshot.
///
/// Pipeline: re-normalize from pinned bytes -> resolve provider files -> resolve runtime/loader ->
/// ingest embedded managed files -> compose pack payload onto the base install plan -> validate ->
/// fingerprint. No committed instance state is touched before execution.
pub(crate) async fn plan_import(
    context: &Arc<ServiceContext>,
    request: &ModpackImportRequest,
    operation: &OperationController,
) -> Result<ModpackImportPlan> {
    operation.set_stage("validate-inspection")?;
    request.target.validate().map_err(|source| {
        GrapheneError::new(
            ErrorCode::PackPlanInvalid,
            ErrorKind::Modpack,
            "import target instance spec is invalid",
        )
        .with_source(source)
    })?;
    reject_existing_target(context, request.target.id)?;

    operation.set_stage("resolve-pack-files")?;
    let archive_path = tokio::task::spawn_blocking({
        let context = Arc::clone(context);
        let snapshot = request.snapshot.clone();
        move || open_cached_snapshot_path(&context, &snapshot)
    })
    .await
    .map_err(join_error)??;

    let cancellation = operation.handle().cancellation_token();
    let policy = request.optional_policy.clone();
    let normalize_path = archive_path.clone();
    let mut normalized = tokio::task::spawn_blocking(move || -> Result<NormalizedModpack> {
        let file = std::fs::File::open(&normalize_path)
            .map_err(|source| stale_error().with_source(source))?;
        let mut index = PackArchiveIndex::open(file, &cancellation).map_err(GrapheneError::from)?;
        let detection =
            graphene_modpack::detect_pack_format(&mut index, false).map_err(GrapheneError::from)?;
        normalize_with_policy(
            &mut index,
            &detection.format,
            &detection.root_prefix,
            &policy,
        )
    })
    .await
    .map_err(join_error)??;
    normalized.sort_for_stability();

    // Exact provider catalog resolution through the existing ContentProvider boundary.
    operation.set_stage("resolve-provider-files")?;
    let resolved = if normalized.pending_provider_files().is_empty() {
        Vec::new()
    } else {
        resolve_pending_provider_files(context, normalized.pending_provider_files(), operation)
            .await?
    };

    // Runtime convergence through the ordinary component pipeline.
    operation.set_stage("resolve-runtime")?;
    let runtime = normalized.runtime();
    let mut install_plan = match runtime.primary_loader() {
        Some(loader) => {
            let selection = LoaderSelection {
                kind: loader.kind(),
                version: LoaderVersionSelector::Exact(LoaderVersion::new(loader.version())?),
            };
            let component_request = ComponentInstallRequest::with_instance(
                request.target.clone(),
                MinecraftVersionId::new(runtime.minecraft_version())?,
                selection,
            );
            plan_component_install(Arc::clone(context), component_request, operation).await?
        }
        None => {
            vanilla_plan(
                context,
                runtime.minecraft_version(),
                &request.target,
                operation,
            )
            .await?
        }
    };
    reject_existing_target(context, request.target.id)?;

    // Embedded managed payload ingestion into the immutable cache.
    operation.set_stage("prepare-embedded-files")?;
    let embedded_ids = ingest_embedded_files(
        context,
        normalized.embedded_files(),
        &archive_path,
        operation,
    )
    .await?;

    // Compose the pack payload onto the validated base plan.
    operation.set_stage("build-install-plan")?;
    compose_pack_payload(
        &mut install_plan,
        &normalized,
        &resolved,
        &embedded_ids,
        &request.snapshot,
    )?;

    operation.set_stage("validate-plan")?;
    install_plan.validate()?;
    reject_existing_target(context, request.target.id)?;

    let origin = build_origin(&normalized, request.snapshot.sha256())?;
    let fingerprint = fingerprint_plan(&install_plan, &origin);

    Ok(ModpackImportPlan {
        schema_version: MODPACK_IMPORT_PLAN_VERSION,
        pack_format: normalized.format(),
        origin,
        fingerprint,
        install_plan,
    })
}

fn normalize_with_policy<R: std::io::Read + std::io::Seek>(
    index: &mut PackArchiveIndex<R>,
    format: &PackFormat,
    root_prefix: &str,
    policy: &OptionalSelectionPolicy,
) -> Result<NormalizedModpack> {
    let result: graphene_modpack::PackResult<NormalizedModpack> = match format {
        PackFormat::Modrinth => {
            graphene_modpack::format::modrinth::normalize(index, root_prefix, policy)
        }
        PackFormat::CurseForge => {
            graphene_modpack::format::curseforge::normalize(index, root_prefix, policy)
        }
        _ => Err(graphene_modpack::PackError::new(
            ErrorCode::PackFormatUnknown,
            "detected pack format has no planning adapter in this build",
        )),
    };
    result.map_err(GrapheneError::from)
}

async fn vanilla_plan(
    context: &Arc<ServiceContext>,
    minecraft_version: &str,
    target: &NewInstanceSpec,
    operation: &OperationController,
) -> Result<InstallPlan> {
    let provider = MojangProvider::new(context.network.clone(), context.provider_config.clone())?;
    let metadata = ServiceMetadataAcquirer::new(Arc::clone(context));
    let rules = current_rule_context(context.platform.os, context.platform.architecture);
    let bundle = provider
        .resolve(
            &MinecraftVersionId::new(minecraft_version)?,
            &rules,
            &metadata,
            operation,
        )
        .await?;
    let install_request =
        InstallRequest::with_instance(target.clone(), MinecraftVersionId::new(minecraft_version)?);
    InstallPlan::build(install_request, bundle.minecraft, bundle.metadata_artifacts)
}

async fn ingest_embedded_files(
    context: &Arc<ServiceContext>,
    embedded: &[EmbeddedPackFile],
    archive_path: &std::path::Path,
    operation: &OperationController,
) -> Result<HashMap<String, ArtifactId>> {
    let total = embedded.len() as u64;
    operation.set_progress(Progress::Items {
        completed: 0,
        total: Some(total),
    })?;

    let mut ids = HashMap::new();
    for (index, file) in embedded.iter().enumerate() {
        if operation.is_cancelled() {
            return Err(cancelled());
        }
        let id = ingest_one_embedded(context, file, archive_path, operation).await?;
        ids.insert(file.archive_entry().to_owned(), id);
        operation.set_progress(Progress::Items {
            completed: index as u64 + 1,
            total: Some(total),
        })?;
    }
    Ok(ids)
}

async fn ingest_one_embedded(
    context: &Arc<ServiceContext>,
    file: &EmbeddedPackFile,
    archive_path: &std::path::Path,
    operation: &OperationController,
) -> Result<ArtifactId> {
    let artifact_id = artifact_id_for_digest(file.sha256());
    let integrity = ArtifactIntegrity::none().with_sha256(*file.sha256());
    let cache = context.storage.cache_address(&integrity)?;
    if context.storage.committed_file_exists(cache.path())? {
        return Ok(artifact_id);
    }

    let scratch = context
        .storage
        .download_temp_path(artifact_id, operation.handle().id())?;
    let context = Arc::clone(context);
    let archive_path = archive_path.to_path_buf();
    let entry = file.archive_entry().to_owned();
    let expected_size = file.size();
    let expected_digest = *file.sha256();
    let destination = cache.path().to_path_buf();
    let cancellation = operation.handle().cancellation_token();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let open = std::fs::File::open(&archive_path)
            .map_err(|source| stale_error().with_source(source))?;
        let mut index = PackArchiveIndex::open(open, &cancellation).map_err(GrapheneError::from)?;

        if let Some(parent) = scratch.parent() {
            std::fs::create_dir_all(parent).map_err(|source| {
                GrapheneError::new(
                    ErrorCode::DirectoryCreateFailed,
                    ErrorKind::Filesystem,
                    "failed to create ingestion scratch directory",
                )
                .with_source(source)
            })?;
        }
        let output = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&scratch)
            .map_err(|source| {
                GrapheneError::new(
                    ErrorCode::FileOpenFailed,
                    ErrorKind::Filesystem,
                    "failed to create embedded ingestion scratch file",
                )
                .with_source(source)
            })?;
        let mut writer = HashingOutput {
            inner: output,
            hasher: sha2::Sha256::new(),
        };
        let streamed = index
            .stream_entry_hashed(&entry, &mut writer, &cancellation)
            .map_err(GrapheneError::from)?;
        let observed: [u8; 32] = writer.hasher.finalize().into();

        if streamed.size != expected_size || observed != *expected_digest.as_bytes() {
            return Err(GrapheneError::new(
                ErrorCode::HashMismatch,
                ErrorKind::Modpack,
                "embedded payload changed between inspection and ingestion",
            ));
        }

        context.storage.commit_verified(&scratch, &destination)?;
        let _ = std::fs::remove_file(&scratch);
        Ok(())
    })
    .await
    .map_err(join_error)??;

    Ok(artifact_id)
}

struct HashingOutput<W: std::io::Write> {
    inner: W,
    hasher: sha2::Sha256,
}

impl<W: std::io::Write> std::io::Write for HashingOutput<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.hasher.update(buf);
        self.inner.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Appends pack-managed artifacts, ordered seed layers, initial content, and pack origin onto a
/// validated base plan, enforcing the complete collision map before staging can begin.
fn compose_pack_payload(
    install_plan: &mut InstallPlan,
    normalized: &NormalizedModpack,
    resolved: &[ResolvedPackFile],
    embedded_ids: &HashMap<String, ArtifactId>,
    snapshot: &PackSourceSnapshot,
) -> Result<()> {
    let mut case_folded = install_plan
        .shared_materializations
        .iter()
        .chain(&install_plan.instance_materializations)
        .map(|materialization| materialization.destination.as_str().to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut declared = HashMap::<ArtifactId, ()>::new();

    let append_managed = |plan: &mut InstallPlan,
                          case_folded: &mut BTreeSet<String>,
                          declared: &mut HashMap<ArtifactId, ()>,
                          artifact: Artifact,
                          destination: String|
     -> Result<()> {
        let full = format!("{MANAGED_DESTINATION_PREFIX}{destination}");
        if !case_folded.insert(full.to_ascii_lowercase()) {
            return Err(GrapheneError::new(
                ErrorCode::PackPlanInvalid,
                ErrorKind::Modpack,
                "pack destination collides with an existing managed destination",
            )
            .with_context("destination", full));
        }
        let artifact_id = artifact.id;
        if declared.insert(artifact_id, ()).is_some() {
            return Err(GrapheneError::new(
                ErrorCode::PackPlanInvalid,
                ErrorKind::Modpack,
                "pack artifact identity is reused",
            ));
        }
        plan.artifacts.push(PlannedArtifact { artifact });
        plan.instance_materializations.push(Materialization {
            artifact_id,
            destination: ManagedPath::new(&full)?,
            scope: MaterializationScope::Instance,
        });
        Ok(())
    };

    // Remote managed files selected by the host policy.
    for file in normalized.managed_files() {
        if matches!(file.selection(), FileSelection::ExcludedForClient) {
            continue;
        }
        let id = managed_artifact_id(file.destination().as_str(), file.integrity());
        append_managed(
            install_plan,
            &mut case_folded,
            &mut declared,
            Artifact {
                id,
                kind: ArtifactKind::Binary,
                sources: file
                    .sources()
                    .iter()
                    .map(|source| ArtifactSource::new(source.as_str().to_owned()))
                    .collect(),
                integrity: file.integrity().clone(),
                expected_size: Some(file.size()),
                cache_policy: CachePolicy::UseVerified,
            },
            file.destination().as_str().to_owned(),
        )?;
    }

    // Provider-resolved files land as enabled mods under mods/ (exact catalog identity preserved).
    for resolved_file in resolved {
        if matches!(resolved_file.selection(), FileSelection::ExcludedForClient) {
            continue;
        }
        let content_file = resolved_file.file();
        let destination = format!("{MODS_DIRECTORY}/{}", content_file.filename);
        let id = managed_artifact_id(&destination, &content_file.integrity);
        append_managed(
            install_plan,
            &mut case_folded,
            &mut declared,
            Artifact {
                id,
                kind: ArtifactKind::Binary,
                sources: content_file.sources.clone(),
                integrity: content_file.integrity.clone(),
                expected_size: Some(content_file.size),
                cache_policy: CachePolicy::UseVerified,
            },
            destination,
        )?;
    }

    // Embedded promoted files are cache-only by construction.
    for embedded in normalized.embedded_files() {
        let id = embedded_ids
            .get(embedded.archive_entry())
            .copied()
            .ok_or_else(|| {
                GrapheneError::new(
                    ErrorCode::PackPlanInvalid,
                    ErrorKind::Modpack,
                    "embedded file was not ingested during planning",
                )
            })?;
        append_managed(
            install_plan,
            &mut case_folded,
            &mut declared,
            Artifact {
                id,
                kind: ArtifactKind::Binary,
                sources: Vec::new(),
                integrity: ArtifactIntegrity::none().with_sha256(*embedded.sha256()),
                expected_size: Some(embedded.size()),
                cache_policy: CachePolicy::UseVerified,
            },
            embedded.destination().as_str().to_owned(),
        )?;
    }

    // Ordered seed layers extracted from the cached snapshot during execution.
    let snapshot_id = snapshot_artifact(snapshot).id;
    for entry in normalized.seed_entries() {
        let full = format!(
            "{MANAGED_DESTINATION_PREFIX}{}",
            entry.destination().as_str()
        );
        if case_folded.contains(&full.to_ascii_lowercase()) {
            return Err(GrapheneError::new(
                ErrorCode::PackPlanInvalid,
                ErrorKind::Modpack,
                "seed destination collides with a managed destination",
            )
            .with_context("destination", full));
        }
        case_folded.insert(full.to_ascii_lowercase());
        install_plan.seed_archive_layers.push(SeedArchiveLayer::new(
            snapshot_id,
            entry.archive_entry(),
            ManagedPath::new(&full)?,
            entry.size(),
            *entry.sha256(),
        )?);
    }

    // Initial managed-content entries for mod payloads.
    install_plan.initial_content = build_initial_content(normalized, resolved)?;

    // Historical provenance; never carries URLs or host paths.
    install_plan.pack_origin = Some(build_origin(normalized, snapshot.sha256())?);

    Ok(())
}

fn build_initial_content(
    normalized: &NormalizedModpack,
    resolved: &[ResolvedPackFile],
) -> Result<Vec<LockedContentEntry>> {
    let mut entries = Vec::new();

    for resolved_file in resolved {
        if matches!(resolved_file.selection(), FileSelection::ExcludedForClient) {
            continue;
        }
        let destination = format!("{MODS_DIRECTORY}/{}", resolved_file.file().filename);
        entries.push(content_entry(
            &destination,
            Some(resolved_file.pending().provider_ref()),
            resolved_file.version().dependencies.as_slice(),
        )?);
    }

    for embedded in normalized.embedded_files() {
        if !is_mod_destination(embedded.destination()) {
            continue;
        }
        entries.push(content_entry(embedded.destination().as_str(), None, &[])?);
    }

    entries.sort_by(|a, b| a.entry_id.cmp(&b.entry_id));
    entries.dedup_by(|a, b| a.entry_id == b.entry_id);
    Ok(entries)
}

fn content_entry(
    destination: &str,
    provider_ref: Option<&ProviderFileRef>,
    dependencies: &[ContentDependency],
) -> Result<LockedContentEntry> {
    let full_destination = format!("{MANAGED_DESTINATION_PREFIX}{destination}");
    let deps = dependencies
        .iter()
        .take(16)
        .map(|dependency| {
            let mut target = match &dependency.target {
                DependencyTarget::Project(project) => project.project_id.clone(),
                DependencyTarget::Version(version) => {
                    format!("{}:{}", version.project_id, version.version_id)
                }
                DependencyTarget::FilenameHint(name) => name.clone(),
            };
            if target.chars().count() > MAX_DISPLAY_STRING_CHARS {
                target.truncate(MAX_DISPLAY_STRING_CHARS);
            }
            LockedContentDependency {
                target,
                relation: relation_string(dependency.relation),
            }
        })
        .collect();

    Ok(LockedContentEntry {
        entry_id: content_key(destination),
        kind: "MOD".to_owned(),
        provider: provider_ref.map(|_| "curseforge".to_owned()),
        project_id: provider_ref
            .map(ProviderFileRef::project_id)
            .map(str::to_owned),
        version_id: provider_ref
            .map(ProviderFileRef::file_id)
            .map(str::to_owned),
        file_id: provider_ref
            .map(ProviderFileRef::file_id)
            .map(str::to_owned),
        artifact_logical_key: format!("{CONTENT_KEY_PREFIX}{full_destination}"),
        destination: ManagedRelativePath::new(full_destination.clone()).map_err(|source| {
            GrapheneError::new(
                ErrorCode::PackPlanInvalid,
                ErrorKind::Modpack,
                "content destination is invalid",
            )
            .with_source(source)
        })?,
        enabled: true,
        dependencies: deps,
    })
}

fn relation_string(relation: graphene_content::DependencyRelation) -> String {
    use graphene_content::DependencyRelation as R;
    match relation {
        R::Required => "required".to_owned(),
        R::Optional => "optional".to_owned(),
        R::Incompatible => "incompatible".to_owned(),
        R::Embedded => "embedded".to_owned(),
        _ => "unknown".to_owned(),
    }
}

fn content_key(destination: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(destination.as_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    let mut hex = String::with_capacity(19);
    hex.push_str("pk-");
    for byte in &digest[..8] {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

fn managed_artifact_id(destination: &str, integrity: &ArtifactIntegrity) -> ArtifactId {
    let digest = integrity
        .sha256()
        .map(|d| d.to_string())
        .or_else(|| integrity.sha1().map(|d| d.to_string()))
        .unwrap_or_default();
    let combined = format!("{destination}\u{0}{digest}");

    let mut hasher = sha2::Sha256::new();
    hasher.update(combined.as_bytes());
    let sum: [u8; 32] = hasher.finalize().into();
    let mut id_bytes = [0_u8; 16];
    id_bytes.copy_from_slice(&sum[..16]);
    ArtifactId::from_bytes(id_bytes)
}

fn build_origin(
    normalized: &NormalizedModpack,
    source_sha256: &Sha256Digest,
) -> Result<LockedPackOrigin> {
    LockedPackOrigin::new(
        normalized.format().as_str().to_ascii_uppercase(),
        bounded_optional(normalized.metadata().name()),
        bounded_optional_version(normalized.metadata().version()),
        *source_sha256,
        None,
        None,
    )
    .map_err(|source| {
        GrapheneError::new(
            ErrorCode::PackPlanInvalid,
            ErrorKind::Modpack,
            "pack origin could not be represented safely",
        )
        .with_source(source)
    })
}

fn bounded_optional(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_DISPLAY_STRING_CHARS / 2 {
        return None;
    }
    Some(trimmed.to_owned())
}

fn bounded_optional_version(value: Option<&str>) -> Option<String> {
    value.and_then(bounded_optional)
}

/// Deterministic semantic fingerprint over authoritative plan inputs.
fn fingerprint_plan(plan: &InstallPlan, origin: &LockedPackOrigin) -> Sha256Digest {
    #[derive(Serialize)]
    struct ManagedEntry {
        destination: String,
        size: Option<u64>,
        sha256: Option<String>,
        sha1: Option<String>,
    }
    #[derive(Serialize)]
    struct FingerprintInput {
        plan_version: u32,
        origin_format: String,
        origin_sha256: String,
        instance_id: String,
        minecraft_version: String,
        components: Vec<(String, String)>,
        managed: Vec<ManagedEntry>,
        seeds: Vec<(String, String, u64, String)>,
        content_keys: Vec<String>,
    }

    let mut managed = Vec::new();
    for materialization in &plan.instance_materializations {
        let artifact = plan
            .artifacts
            .iter()
            .find(|p| p.artifact.id == materialization.artifact_id)
            .map(|p| &p.artifact);
        let (size, sha256, sha1) = artifact
            .map(|a| {
                (
                    a.expected_size,
                    a.integrity.sha256().map(|d| d.to_string()),
                    a.integrity.sha1().map(|d| d.to_string()),
                )
            })
            .unwrap_or((None, None, None));
        managed.push(ManagedEntry {
            destination: materialization.destination.as_str().to_owned(),
            size,
            sha256,
            sha1,
        });
    }
    managed.sort_by(|a, b| a.destination.cmp(&b.destination));

    let mut seeds = Vec::new();
    for layer in &plan.seed_archive_layers {
        seeds.push((
            layer.destination.as_str().to_owned(),
            layer.archive_entry.clone(),
            layer.expected_size,
            layer.expected_sha256.to_string(),
        ));
    }
    seeds.sort();

    let input = FingerprintInput {
        plan_version: plan.plan_version,
        origin_format: origin.format.clone(),
        origin_sha256: origin.source_sha256.to_string(),
        instance_id: plan.instance.descriptor.instance_id.to_string(),
        minecraft_version: plan.receipt.requested_version.clone(),
        components: plan
            .receipt
            .components
            .iter()
            .map(|c| (c.uid.clone(), c.version.clone()))
            .collect(),
        managed,
        seeds,
        content_keys: plan
            .initial_content
            .iter()
            .map(|e| e.entry_id.clone())
            .collect(),
    };

    let json = serde_json::to_vec(&input).unwrap_or_default();
    Sha256Digest::from_bytes(sha2::Sha256::digest(&json).into())
}

fn reject_existing_target(context: &Arc<ServiceContext>, id: InstanceId) -> Result<()> {
    let target = graphene_storage::InstancePaths::new(context.storage.path()).instance_root(id);
    match std::fs::symlink_metadata(&target) {
        Ok(_) => Err(GrapheneError::new(
            ErrorCode::InstallTargetExists,
            ErrorKind::Install,
            "import target instance already exists",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(GrapheneError::new(
            ErrorCode::InstallStageFailed,
            ErrorKind::Install,
            "failed to inspect import target",
        )
        .with_source(source)),
    }
}

fn cancelled() -> GrapheneError {
    GrapheneError::new(
        ErrorCode::DownloadCancelled,
        ErrorKind::Cancelled,
        "pack planning was cancelled",
    )
}

fn join_error(source: tokio::task::JoinError) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::InternalInvariantViolation,
        ErrorKind::Internal,
        "blocking planning worker failed",
    )
    .with_source(source)
}

fn stale_error() -> GrapheneError {
    GrapheneError::new(
        ErrorCode::PackPlanStale,
        ErrorKind::Modpack,
        "pinned pack source snapshot is no longer available",
    )
}
