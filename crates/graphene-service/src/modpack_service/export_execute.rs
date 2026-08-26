//! Execution half of the modpack export pipeline.
//!
//! Snapshots verified instance/cache bytes, builds the deterministic Graphene pack v1
//! archive, validates it against its own import contract, and publishes create-only.

use super::export::{
    MANIFEST_ENTRY_NAME, ModpackExportPlan, ModpackExportResult, PlannedManagedFile,
    fingerprint_plan, modpack_error, read_lockfile,
};
use crate::context::ServiceContext;
use graphene_core::{
    ErrorCode, ErrorKind, GrapheneError, OperationController, Result, Sha1Digest, Sha256Digest,
    Sha512Digest,
};
use sha2::Digest as _;
use std::{collections::BTreeSet, path::Path, sync::Arc};

pub(crate) async fn execute_export(
    context: &Arc<ServiceContext>,
    plan: &ModpackExportPlan,
    operation: &OperationController,
) -> Result<ModpackExportResult> {
    operation.set_stage("acquire-instance-lease")?;
    let repo = crate::InstanceRepository::new(context.storage.path());
    let _lease = repo.acquire_shared_lease(*plan.instance_id())?;

    // Stale-source protection: the committed desired state must still fingerprint identically.
    let lockfile = read_lockfile(context, *plan.instance_id())?;
    let recomputed = fingerprint_plan(
        plan.instance_id(),
        &lockfile.minecraft_version,
        &lockfile.components,
        &plan.managed,
        &plan.seeds,
    );
    if !lockfile.minecraft_version.eq(&plan.minecraft_version) || recomputed != plan.fingerprint {
        return Err(modpack_error(
            ErrorCode::PackExportStale,
            "instance state changed after the export plan was created",
        ));
    }

    if context
        .storage
        .path()
        .join("instances")
        .file_name()
        .is_none()
    {
        return Err(modpack_error(
            ErrorCode::PackExportInvalid,
            "data root is not initialized",
        ));
    }
    if plan.output.exists() || std::fs::symlink_metadata(&plan.output).is_ok() {
        return Err(modpack_error(
            ErrorCode::PackExportInvalid,
            "export destination already exists; publication is create-only",
        ));
    }

    operation.set_stage("snapshot-export-files")?;
    let minecraft_dir = repo.paths().minecraft_dir(*plan.instance_id());
    let cancellation = operation.handle().cancellation_token();
    let staging = tokio::task::spawn_blocking({
        let plan = plan.clone();
        let context = Arc::clone(context);
        let minecraft_dir = minecraft_dir.clone();
        move || snapshot_export_files(&context, &plan, &minecraft_dir, &cancellation)
    })
    .await
    .map_err(|source| {
        GrapheneError::new(
            ErrorCode::InternalInvariantViolation,
            ErrorKind::Internal,
            "blocking export worker failed",
        )
        .with_source(source)
    })??;

    operation.set_stage("build-pack-manifest")?;
    let manifest = build_manifest_json(plan, &staging)?;

    operation.set_stage("write-export-archive")?;
    let mut entries: BTreeSet<String> = BTreeSet::new();
    entries.insert(MANIFEST_ENTRY_NAME.to_owned());
    entries.extend(staging.objects.keys().cloned());
    entries.extend(staging.seeds.keys().cloned());

    let mut writer =
        graphene_core::archive::DeterministicZipWriter::new(std::io::Cursor::new(Vec::new()));
    for name in &entries {
        let payload: &[u8] = if name == MANIFEST_ENTRY_NAME {
            &manifest
        } else if let Some(payload) = staging.objects.get(name) {
            payload
        } else {
            staging.seeds.get(name).ok_or_else(|| {
                modpack_error(ErrorCode::PackExportInvalid, "staged payload vanished")
            })?
        };
        writer.add_entry(name, payload).map_err(|error| {
            modpack_error(
                ErrorCode::PackExportInvalid,
                format!("deterministic archive rejected entry '{name}': {error}"),
            )
        })?;
    }
    writer.finish().map_err(|error| {
        modpack_error(
            ErrorCode::PackExportInvalid,
            format!("deterministic archive finalize failed: {error}"),
        )
    })?;
    let archive = writer.into_inner().expect("finalized").into_inner();

    operation.set_stage("validate-export-archive")?;
    validate_archive_snapshot(&archive, plan)?;

    operation.set_stage("pre-commit")?;
    if plan.output.exists() {
        return Err(modpack_error(
            ErrorCode::PackExportInvalid,
            "export destination appeared during execution; refusing to overwrite",
        ));
    }

    operation.set_stage("commit")?;
    publish_create_only(&plan.output, &archive)?;

    Ok(ModpackExportResult {
        output: plan.output.clone(),
        archive_sha256: hash_file_sync(&plan.output)?,
        archive_size: archive.len() as u64,
        referenced_managed_files: plan.managed.len(),
        embedded_objects: staging.objects.len(),
        seed_files: staging.seeds.len(),
    })
}

struct ExportStaging {
    objects: std::collections::BTreeMap<String, Vec<u8>>,
    seeds: std::collections::BTreeMap<String, Vec<u8>>,
    /// Observed `(sha256, size)` per managed destination recorded during snapshotting.
    managed_observed: std::collections::BTreeMap<String, (String, u64)>,
}

fn snapshot_export_files(
    context: &Arc<ServiceContext>,
    plan: &ModpackExportPlan,
    minecraft_dir: &Path,
    cancellation: &graphene_core::CancellationToken,
) -> Result<ExportStaging> {
    let mut objects = std::collections::BTreeMap::new();
    let mut seeds = std::collections::BTreeMap::new();
    let mut managed_observed = std::collections::BTreeMap::new();

    for file in &plan.managed {
        if cancellation.is_cancelled() {
            return Err(modpack_error(
                ErrorCode::OperationCancelled,
                "export was cancelled",
            ));
        }
        let observed = read_verified_managed(context, plan, file, minecraft_dir)?;
        if file.embed {
            let hex = observed.sha256.to_string();
            let entry = format!("objects/sha256/{}/{}", &hex[..2], hex);
            objects.insert(entry, observed.bytes);
        }
        managed_observed.insert(
            file.destination.clone(),
            (observed.sha256.to_string(), observed.size),
        );
    }

    for seed in &plan.seeds {
        if cancellation.is_cancelled() {
            return Err(modpack_error(
                ErrorCode::OperationCancelled,
                "export was cancelled",
            ));
        }
        let absolute = minecraft_dir.join(&seed.relative);
        let bytes = std::fs::read(&absolute).map_err(|source| {
            modpack_error(
                ErrorCode::PackExportStale,
                format!("seed file '{}' changed or disappeared", seed.relative),
            )
            .with_source(source)
        })?;
        if bytes.len() as u64 != seed.size {
            return Err(modpack_error(
                ErrorCode::PackExportStale,
                format!("seed file '{}' changed size after planning", seed.relative),
            ));
        }
        let digest = Sha256Digest::from_bytes(sha2::Sha256::digest(&bytes).into());
        if digest.to_string() != seed.sha256 {
            return Err(modpack_error(
                ErrorCode::PackExportStale,
                format!(
                    "seed file '{}' no longer matches its planned hash",
                    seed.relative
                ),
            ));
        }
        seeds.insert(format!("seed/{}", seed.relative), bytes);
    }

    Ok(ExportStaging {
        objects,
        seeds,
        managed_observed,
    })
}

struct ObservedManaged {
    bytes: Vec<u8>,
    sha256: Sha256Digest,
    size: u64,
}

fn read_verified_managed(
    context: &Arc<ServiceContext>,
    _plan: &ModpackExportPlan,
    file: &PlannedManagedFile,
    minecraft_dir: &Path,
) -> Result<ObservedManaged> {
    let instance_copy = minecraft_dir.join(&file.destination);
    let source_path = if instance_copy.is_file() {
        instance_copy
    } else if let Some(integrity) = &file.cache_integrity {
        let address = context.storage.cache_address(integrity)?;
        address.path().to_path_buf()
    } else {
        return Err(modpack_error(
            ErrorCode::PackExportStale,
            format!(
                "managed artifact '{}' is neither materialized nor cached",
                file.destination
            ),
        ));
    };

    let bytes = std::fs::read(&source_path).map_err(|source| {
        modpack_error(
            ErrorCode::PackExportStale,
            format!("managed artifact '{}' is unreadable", file.destination),
        )
        .with_source(source)
    })?;
    if let Some(expected) = file.size
        && bytes.len() as u64 != expected
    {
        return Err(modpack_error(
            ErrorCode::PackExportStale,
            format!("managed artifact '{}' changed size", file.destination),
        ));
    }
    // Verify every declared strong digest against the observed bytes.
    if let Some(declared) = &file.declared_sha1 {
        let mut hasher = sha1::Sha1::new();
        hasher.update(&bytes);
        let digest_bytes: [u8; 20] = hasher.finalize().into();
        let declared_digest = declared.parse::<Sha1Digest>().map_err(|_| {
            modpack_error(
                ErrorCode::PackExportInvalid,
                "planned SHA-1 identity is malformed",
            )
        })?;
        if Sha1Digest::from_bytes(digest_bytes) != declared_digest {
            return Err(modpack_error(
                ErrorCode::PackExportStale,
                format!(
                    "managed artifact '{}' failed its declared SHA-1",
                    file.destination
                ),
            ));
        }
    }
    if let Some(declared) = &file.declared_sha512 {
        let mut hasher = sha2::Sha512::new();
        hasher.update(&bytes);
        let digest_bytes: [u8; 64] = hasher.finalize().into();
        let declared_digest = declared.parse::<Sha512Digest>().map_err(|_| {
            modpack_error(
                ErrorCode::PackExportInvalid,
                "planned SHA-512 identity is malformed",
            )
        })?;
        if Sha512Digest::from_bytes(digest_bytes) != declared_digest {
            return Err(modpack_error(
                ErrorCode::PackExportStale,
                format!(
                    "managed artifact '{}' failed its declared SHA-512",
                    file.destination
                ),
            ));
        }
    }

    let sha256_bytes: [u8; 32] = sha2::Sha256::digest(&bytes).into();
    let size = bytes.len() as u64;
    Ok(ObservedManaged {
        bytes,
        sha256: Sha256Digest::from_bytes(sha256_bytes),
        size,
    })
}

fn build_manifest_json(plan: &ModpackExportPlan, staging: &ExportStaging) -> Result<Vec<u8>> {
    use serde_json::json;

    // Embedded object entry per destination, derived from the observed identity recorded
    // during snapshotting; staged objects are addressed by their content hash.
    let embedded_by_destination: std::collections::BTreeMap<&str, String> = plan
        .managed
        .iter()
        .filter(|file| file.embed)
        .filter_map(|file| {
            let (sha_hex, _) = staging.managed_observed.get(file.destination.as_str())?;
            Some((
                file.destination.as_str(),
                format!("objects/sha256/{}/{}", &sha_hex[..2], sha_hex),
            ))
        })
        .collect();

    let mut managed_files = Vec::with_capacity(plan.managed.len());
    for file in &plan.managed {
        // Manifest identity always comes from observed snapshot bytes, never from planning
        // claims, so the written pack is self-consistent by construction.
        let Some((sha_hex, size)) = staging.managed_observed.get(&file.destination) else {
            return Err(modpack_error(
                ErrorCode::PackExportInvalid,
                format!(
                    "managed artifact '{}' lacks an observed export identity",
                    file.destination
                ),
            ));
        };
        let object_entry = embedded_by_destination
            .get(file.destination.as_str())
            .map(String::as_str);

        let mut entry = serde_json::Map::new();
        entry.insert("destination".to_owned(), json!(file.destination));
        entry.insert("sha256".to_owned(), json!(sha_hex));
        entry.insert("size".to_owned(), json!(size));

        // The v1 import contract requires exactly one acquisition strategy per managed
        // file. Provider provenance supersedes persisted URLs because resolution yields
        // fresh download locations at import time.
        let provenance = match (
            file.provider.as_deref(),
            file.project_id.as_deref(),
            file.version_id.as_deref(),
        ) {
            (Some(provider), Some(project), Some(version)) if provider == "curseforge" => {
                Some((provider.to_owned(), project.to_owned(), version.to_owned()))
            }
            _ => None,
        };

        if !file.sources.is_empty() && object_entry.is_none() && provenance.is_none() {
            entry.insert("sources".to_owned(), json!(file.sources));
        }
        if let Some(entry_name) = object_entry {
            entry.insert(
                "embedded_object".to_owned(),
                json!(object_sha_hex(entry_name)),
            );
        }
        if let Some((provider, project, version)) = provenance {
            entry.insert(
                "content_provenance".to_owned(),
                json!({
                    "provider": provider,
                    "project_id": project,
                    "version_id": version,
                }),
            );
        }
        managed_files.push(serde_json::Value::Object(entry));
    }

    let seed_files: Vec<serde_json::Value> = plan
        .seeds
        .iter()
        .map(|seed| {
            json!({
                "destination": seed.relative,
                "sha256": seed.sha256,
                "size": seed.size,
                "archive_entry": format!("seed/{}", seed.relative),
            })
        })
        .collect();

    let mut runtime = serde_json::Map::new();
    runtime.insert(
        "minecraft_version".to_owned(),
        json!(plan.minecraft_version),
    );
    if let Some((kind, version)) = &plan.primary_loader {
        runtime.insert(
            "primary_loader".to_owned(),
            json!({"kind": kind, "exact_version": version}),
        );
    }

    let mut pack = serde_json::Map::new();
    pack.insert("name".to_owned(), json!(plan.metadata.name()));
    if let Some(version) = plan.metadata.version() {
        pack.insert("version".to_owned(), json!(version));
    }
    if let Some(summary) = plan.metadata.summary() {
        pack.insert("summary".to_owned(), json!(summary));
    }

    let manifest = json!({
        "schema_version": 1,
        "pack": serde_json::Value::Object(pack),
        "runtime": serde_json::Value::Object(runtime),
        "managed_files": managed_files,
        "seed_files": seed_files,
    });
    serde_json::to_vec_pretty(&manifest).map_err(|source| {
        modpack_error(
            ErrorCode::PackExportInvalid,
            "manifest serialization failed",
        )
        .with_source(source)
    })
}

fn object_sha_hex(entry: &str) -> String {
    entry.rsplit('/').next().unwrap_or_default().to_owned()
}

fn validate_archive_snapshot(archive: &[u8], plan: &ModpackExportPlan) -> Result<()> {
    let mut index = graphene_modpack::PackArchiveIndex::open(
        std::io::Cursor::new(archive.to_vec()),
        &graphene_core::CancellationToken::new(),
    )
    .map_err(|error| {
        modpack_error(
            ErrorCode::PackExportInvalid,
            format!("written export archive failed to reopen: {error}"),
        )
    })?;
    let detection = graphene_modpack::detect_pack_format(&mut index, false).map_err(|error| {
        modpack_error(
            ErrorCode::PackExportInvalid,
            format!("written export archive is not a recognizable Graphene pack: {error}"),
        )
    })?;
    if detection.format != graphene_modpack::PackFormat::Graphene {
        return Err(modpack_error(
            ErrorCode::PackExportInvalid,
            "written export archive detected as a foreign format",
        ));
    }
    let normalized =
        graphene_modpack::format::graphene::normalize(&mut index, "").map_err(|error| {
            modpack_error(
                ErrorCode::PackExportInvalid,
                format!("written export archive fails its own import contract: {error}"),
            )
        })?;
    let represented = normalized.managed_files().len()
        + normalized.embedded_files().len()
        + normalized.pending_provider_files().len();
    if represented != plan.managed.len() {
        return Err(modpack_error(
            ErrorCode::PackExportInvalid,
            "written export archive lost managed declarations",
        ));
    }
    if normalized.seed_entries().len() != plan.seeds.len() {
        return Err(modpack_error(
            ErrorCode::PackExportInvalid,
            "written export archive lost seed declarations",
        ));
    }
    Ok(())
}

fn publish_create_only(output: &Path, archive: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|source| {
            modpack_error(
                ErrorCode::PackExportInvalid,
                "create-only export publication failed",
            )
            .with_source(source)
        })?;
    file.write_all(archive).map_err(|source| {
        modpack_error(ErrorCode::PackExportInvalid, "export write failed").with_source(source)
    })?;
    file.sync_all().map_err(|source| {
        modpack_error(ErrorCode::PackExportInvalid, "export sync failed").with_source(source)
    })?;
    Ok(())
}

fn hash_file_sync(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).map_err(|source| {
        modpack_error(
            ErrorCode::PackExportInvalid,
            "published export is unreadable",
        )
        .with_source(source)
    })?;
    let digest: [u8; 32] = sha2::Sha256::digest(&bytes).into();
    Ok(Sha256Digest::from_bytes(digest).to_string())
}
