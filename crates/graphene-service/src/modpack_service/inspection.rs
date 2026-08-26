use super::source::{
    PackSource, open_cached_snapshot_path, snapshot_https_url, snapshot_local_file,
};
use crate::context::ServiceContext;
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, OperationController, Result};
use graphene_modpack::{
    NormalizedModpack, OptionalSelectionPolicy, PackArchiveIndex, PackError, PackFormat,
    PackInspection, PackResult, detect_pack_format,
};
use std::{io::Seek, sync::Arc};

/// Inspects a local file or HTTPS URL as an immutable pack-source snapshot.
///
/// The source is snapshotted into the content-addressed cache first; every later step reads only
/// the pinned bytes, so repeated inspections and planning observe identical content.
pub(crate) async fn inspect_pack(
    context: &Arc<ServiceContext>,
    source: &PackSource,
    operation: &OperationController,
) -> Result<PackInspection> {
    operation.set_stage("snapshot-source")?;
    let snapshot = match source {
        PackSource::LocalFile(path) => snapshot_local_file(context, path, operation).await?,
        PackSource::HttpsUrl(url) => snapshot_https_url(context, url, operation).await?,
    };

    operation.set_stage("index-archive")?;
    let archive_path = tokio::task::spawn_blocking({
        let context = Arc::clone(context);
        let snapshot = snapshot.clone();
        move || open_cached_snapshot_path(&context, &snapshot)
    })
    .await
    .map_err(join_error)??;

    operation.set_stage("detect-format")?;
    let cancellation = operation.handle().cancellation_token();
    let normalized = tokio::task::spawn_blocking(move || -> Result<NormalizedModpack> {
        let file = std::fs::File::open(&archive_path).map_err(|source| {
            GrapheneError::new(
                ErrorCode::PackPlanStale,
                ErrorKind::Modpack,
                "pinned pack snapshot disappeared before inspection",
            )
            .with_source(source)
        })?;
        let mut index = PackArchiveIndex::open(file, &cancellation).map_err(GrapheneError::from)?;

        let detection = detect_pack_format(&mut index, false).map_err(GrapheneError::from)?;
        normalize_detected(&mut index, &detection.format, &detection.root_prefix)
    })
    .await
    .map_err(join_error)??;

    operation.set_stage("normalize-pack")?;
    let inspection = PackInspection::from_normalized(&normalized, snapshot);
    operation.set_stage("inspection-complete")?;
    Ok(inspection)
}

/// Re-normalizes one detected format from the pinned snapshot (shared by inspect and plan).
pub(crate) fn normalize_detected<R: Seek + std::io::Read>(
    index: &mut PackArchiveIndex<R>,
    format: &PackFormat,
    root_prefix: &str,
) -> Result<NormalizedModpack> {
    // Inspection always uses RequiredOnly so choices stay host-visible rather than pre-decided.
    let policy = OptionalSelectionPolicy::RequiredOnly;
    let result: PackResult<NormalizedModpack> = match format {
        PackFormat::Modrinth => {
            graphene_modpack::format::modrinth::normalize(index, root_prefix, &policy)
        }
        PackFormat::CurseForge => {
            graphene_modpack::format::curseforge::normalize(index, root_prefix, &policy)
        }
        PackFormat::PrismMultiMc => normalize_multimc(index, root_prefix),
        PackFormat::Graphene => graphene_modpack::format::graphene::normalize(index, root_prefix),
        PackFormat::Generic => Err(PackError::new(
            ErrorCode::PackFormatUnknown,
            "detected pack format has no import adapter in this build",
        )),
        _ => Err(PackError::new(
            ErrorCode::PackFormatUnknown,
            "detected pack format is not recognized by this engine",
        )),
    };
    result.map_err(GrapheneError::from)
}

fn normalize_multimc<R: Seek + std::io::Read>(
    index: &mut PackArchiveIndex<R>,
    root_prefix: &str,
) -> PackResult<NormalizedModpack> {
    let pack = graphene_modpack::format::multimc::normalize(index, root_prefix)?;
    let metadata = pack.metadata().clone();
    let runtime = pack.runtime().clone();
    let seed_entries = pack.seed_entries().to_vec();

    // Promote embedded mod JARs to provider-neutral managed content; every other payload
    // stays user-mutable seed state.
    let embedded_files: Vec<graphene_modpack::EmbeddedPackFile> = pack
        .embedded_mods()
        .iter()
        .map(|mod_file| {
            let destination = mod_file.destination();
            graphene_modpack::EmbeddedPackFile::new(
                mod_file.archive_entry().to_owned(),
                destination.clone(),
                mod_file.size(),
                *mod_file.sha256(),
                hint_for(destination),
            )
        })
        .collect();
    drop(pack);

    NormalizedModpack::new(
        PackFormat::PrismMultiMc,
        metadata,
        runtime,
        Vec::new(),
        Vec::new(),
        embedded_files,
        seed_entries,
        Vec::new(),
        Vec::new(),
    )
}

/// Content-role hint derived from the pack-relative destination.
fn hint_for(destination: &graphene_modpack::PackPath) -> Option<graphene_modpack::ContentHint> {
    match destination.components().next() {
        Some("mods") => Some(graphene_modpack::ContentHint::Mod),
        Some("resourcepacks") => Some(graphene_modpack::ContentHint::ResourcePack),
        Some("shaderpacks") => Some(graphene_modpack::ContentHint::ShaderPack),
        _ => None,
    }
}

fn join_error(source: tokio::task::JoinError) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::InternalInvariantViolation,
        ErrorKind::Internal,
        "blocking inspection worker failed",
    )
    .with_source(source)
}
