use crate::context::ServiceContext;
use graphene_core::{
    Artifact, ArtifactIntegrity, CachePolicy, ErrorCode, ErrorKind, GrapheneError,
    OperationController, Result, Sha256Digest,
};
use graphene_modpack::PackSourceSnapshot;
use graphene_network::NetworkClient;
use std::{fs, path::Path, sync::Arc};

/// Maximum accepted pack archive size (matches the modpack bounded policy).
const MAX_PACK_SOURCE_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// Host-supplied pack input. Raw paths/URLs never become persisted state: planning pins only the
/// resulting immutable snapshot digest.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PackSource {
    LocalFile(std::path::PathBuf),
    HttpsUrl(String),
}

/// Copies a local pack file into the content-addressed cache while hashing it.
///
/// The source must be an ordinary file (symlinks/special files are rejected), and the original
/// absolute path is discarded once the observed snapshot identity exists.
pub(crate) async fn snapshot_local_file(
    context: &Arc<ServiceContext>,
    path: &Path,
    operation: &OperationController,
) -> Result<PackSourceSnapshot> {
    let metadata = fs::symlink_metadata(path).map_err(|source| {
        GrapheneError::new(
            ErrorCode::PackSourceInvalid,
            ErrorKind::Modpack,
            "pack source file cannot be inspected",
        )
        .with_source(source)
    })?;
    if metadata.is_symlink() || !metadata.is_file() {
        return Err(GrapheneError::new(
            ErrorCode::PackSourceInvalid,
            ErrorKind::Modpack,
            "pack source must be an ordinary local file",
        ));
    }
    if metadata.len() > MAX_PACK_SOURCE_BYTES {
        return Err(GrapheneError::new(
            ErrorCode::PackSourceTooLarge,
            ErrorKind::Modpack,
            "pack source exceeds its byte limit",
        ));
    }

    // Hash off the async runtime: large packs make blocking reads the dominant cost.
    let original = path.to_path_buf();
    let path = original.clone();
    let cancellation = operation.handle().cancellation_token();
    let observed = tokio::task::spawn_blocking(move || hash_local_file(&path, &cancellation))
        .await
        .map_err(join_error)??;

    // Pin the host-supplied bytes into the content-addressed cache: every later stage
    // (inspection, planning) reopens the pack through its pinned snapshot identity, so a
    // local source must become a cached object exactly like an HTTPS transfer.
    let integrity = ArtifactIntegrity::none().with_sha256(observed.sha256);
    let cache = context.storage.cache_address(&integrity)?;
    if !context.storage.committed_file_exists(cache.path())? {
        let scratch = context.storage.download_temp_path(
            artifact_id_for_digest(&observed.sha256),
            operation.handle().id(),
        )?;
        let expected = (observed.bytes, observed.sha256);
        let cancellation = operation.handle().cancellation_token();
        tokio::task::spawn_blocking({
            let original = original.clone();
            let scratch = scratch.clone();
            move || pin_local_copy(&original, &scratch, expected, &cancellation)
        })
        .await
        .map_err(join_error)??;
        context.storage.commit_verified(&scratch, cache.path())?;
    }

    commit_observed(context, operation, observed).await
}

/// Streams one HTTPS URL into the cache as an observed snapshot.
pub(crate) async fn snapshot_https_url(
    context: &Arc<ServiceContext>,
    url: &str,
    operation: &OperationController,
) -> Result<PackSourceSnapshot> {
    let temporary = context
        .storage
        .download_temp_path(artifact_id_for_source(url), operation.handle().id())?;
    let transfer = NetworkClient::download_observed_to(
        &context.network,
        url,
        &temporary,
        MAX_PACK_SOURCE_BYTES,
        operation,
    )
    .await?;
    let observed = ObservedBytes {
        bytes: transfer.bytes,
        sha256: transfer.sha256,
        sha512: transfer.sha512,
        scratch: Some(temporary),
    };
    commit_observed(context, operation, observed).await
}

struct ObservedBytes {
    bytes: u64,
    sha256: Sha256Digest,
    #[allow(dead_code)]
    sha512: graphene_core::Sha512Digest,
    /// Operation-owned scratch file to consume during commit (`None` when the object was
    /// pinned into the cache by a dedicated verified copy, as for local sources).
    scratch: Option<std::path::PathBuf>,
}

async fn commit_observed(
    context: &Arc<ServiceContext>,
    operation: &OperationController,
    mut observed: ObservedBytes,
) -> Result<PackSourceSnapshot> {
    operation.set_stage("snapshot-commit")?;
    checkpoint(operation)?;

    let integrity = ArtifactIntegrity::none().with_sha256(observed.sha256);
    let cache = context.storage.cache_address(&integrity)?;
    let already_cached = context.storage.committed_file_exists(cache.path())?;
    if let (false, Some(scratch)) = (already_cached, observed.scratch.as_ref()) {
        context.storage.commit_verified(scratch, cache.path())?;
    }
    if let Some(scratch) = observed.scratch.take()
        && scratch.exists()
    {
        let _ = fs::remove_file(scratch);
    }

    let snapshot = PackSourceSnapshot::new(observed.sha256, observed.bytes);
    Ok(snapshot)
}

fn hash_local_file(
    path: &Path,
    cancellation: &graphene_core::CancellationToken,
) -> Result<ObservedBytes> {
    use sha2::Digest;
    let file = fs::File::open(path).map_err(|source| {
        GrapheneError::new(
            ErrorCode::PackSourceInvalid,
            ErrorKind::Modpack,
            "pack source file is unreadable",
        )
        .with_source(source)
    })?;
    let len = file
        .metadata()
        .map_err(|source| {
            GrapheneError::new(
                ErrorCode::PackSourceInvalid,
                ErrorKind::Modpack,
                "pack source file length is unavailable",
            )
            .with_source(source)
        })?
        .len();
    if len > MAX_PACK_SOURCE_BYTES {
        return Err(GrapheneError::new(
            ErrorCode::PackSourceTooLarge,
            ErrorKind::Modpack,
            "pack source exceeds its byte limit",
        ));
    }

    let mut reader = std::io::BufReader::with_capacity(1024 * 1024, file);
    let mut sha256 = sha2::Sha256::new();
    let mut sha512 = sha2::Sha512::new();
    let mut remaining = len;
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        if cancellation.is_cancelled() {
            return Err(cancelled());
        }
        let read = std::io::Read::read(&mut reader, &mut buffer).map_err(|source| {
            GrapheneError::new(
                ErrorCode::PackSourceInvalid,
                ErrorKind::Modpack,
                "failed while reading pack source",
            )
            .with_source(source)
        })?;
        if read == 0 {
            break;
        }
        sha256.update(&buffer[..read]);
        sha512.update(&buffer[..read]);
        remaining = remaining.saturating_sub(read as u64);
    }

    let sha256_bytes: [u8; 32] = sha256.finalize().into();
    let sha512_bytes: [u8; 64] = sha512.finalize().into();
    Ok(ObservedBytes {
        bytes: len,
        sha256: graphene_core::Sha256Digest::from_bytes(sha256_bytes),
        sha512: graphene_core::Sha512Digest::from_bytes(sha512_bytes),
        // Never delete the host-supplied original.
        scratch: None,
    })
}

fn join_error(source: tokio::task::JoinError) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::InternalInvariantViolation,
        ErrorKind::Internal,
        "blocking snapshot worker failed",
    )
    .with_source(source)
}

/// Copies a host-supplied pack file into an operation-owned scratch location, verifying
/// the copy against the identity observed from the original before it may be committed.
fn pin_local_copy(
    original: &Path,
    scratch: &Path,
    expected: (u64, Sha256Digest),
    cancellation: &graphene_core::CancellationToken,
) -> Result<()> {
    use sha2::Digest;
    fs::copy(original, scratch).map_err(|source| {
        GrapheneError::new(
            ErrorCode::PackSourceInvalid,
            ErrorKind::Modpack,
            "failed to stage the local pack snapshot",
        )
        .with_source(source)
    })?;

    let file = fs::File::open(scratch).map_err(|source| {
        GrapheneError::new(
            ErrorCode::PackSourceInvalid,
            ErrorKind::Modpack,
            "staged pack snapshot is unreadable",
        )
        .with_source(source)
    })?;
    let mut reader = std::io::BufReader::with_capacity(1024 * 1024, file);
    let mut sha256 = sha2::Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        if cancellation.is_cancelled() {
            return Err(cancelled());
        }
        let read = std::io::Read::read(&mut reader, &mut buffer).map_err(|source| {
            GrapheneError::new(
                ErrorCode::PackSourceInvalid,
                ErrorKind::Modpack,
                "failed while verifying the staged pack snapshot",
            )
            .with_source(source)
        })?;
        if read == 0 {
            break;
        }
        sha256.update(&buffer[..read]);
        bytes = bytes.saturating_add(read as u64);
    }

    let digest_bytes: [u8; 32] = sha256.finalize().into();
    if bytes != expected.0 || Sha256Digest::from_bytes(digest_bytes) != expected.1 {
        let _ = fs::remove_file(scratch);
        return Err(GrapheneError::new(
            ErrorCode::PackSourceInvalid,
            ErrorKind::Modpack,
            "staged pack snapshot does not match its observed source identity",
        ));
    }
    Ok(())
}

fn cancelled() -> GrapheneError {
    GrapheneError::new(
        ErrorCode::DownloadCancelled,
        ErrorKind::Cancelled,
        "pack source snapshot was cancelled",
    )
}

fn checkpoint(operation: &OperationController) -> Result<()> {
    if operation.is_cancelled() {
        Err(cancelled())
    } else {
        Ok(())
    }
}

fn artifact_id_for_source(url: &str) -> graphene_core::ArtifactId {
    // Stable per-URL temp naming so concurrent snapshots of the same URL do not collide on one
    // operation's scratch file.
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(url.as_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    let mut id_bytes = [0_u8; 16];
    id_bytes.copy_from_slice(&digest[..16]);
    graphene_core::ArtifactId::from_bytes(id_bytes)
}

/// Returns the platform path of a cached snapshot, failing typed when it disappeared.
pub(crate) fn open_cached_snapshot_path(
    context: &ServiceContext,
    snapshot: &PackSourceSnapshot,
) -> Result<std::path::PathBuf> {
    let integrity = ArtifactIntegrity::none().with_sha256(*snapshot.sha256());
    let cache = context.storage.cache_address(&integrity)?;
    if !context.storage.committed_file_exists(cache.path())? {
        return Err(GrapheneError::new(
            ErrorCode::PackPlanStale,
            ErrorKind::Modpack,
            "pinned pack source snapshot is no longer available in the cache",
        ));
    }
    Ok(cache.path().to_path_buf())
}

/// Builds the cache-only artifact declaration for a pinned snapshot.
#[must_use]
pub(crate) fn snapshot_artifact(snapshot: &PackSourceSnapshot) -> Artifact {
    // Only the observed SHA-256 is declared: fabricating additional digests would create
    // verification requirements no honest transfer can satisfy.
    Artifact {
        id: artifact_id_for_digest(snapshot.sha256()),
        kind: graphene_core::ArtifactKind::Binary,
        sources: Vec::new(),
        integrity: ArtifactIntegrity::none().with_sha256(*snapshot.sha256()),
        expected_size: Some(snapshot.size()),
        cache_policy: CachePolicy::UseVerified,
    }
}

pub(crate) fn artifact_id_for_digest(digest: &Sha256Digest) -> graphene_core::ArtifactId {
    let bytes = digest.as_bytes();
    let mut id_bytes = [0_u8; 16];
    id_bytes.copy_from_slice(&bytes[..16]);
    graphene_core::ArtifactId::from_bytes(id_bytes)
}
