use crate::{NetworkConfig, ProxyPolicy, retry::retryable_status, verify_transfer};
use futures_util::StreamExt;
use graphene_core::{
    Artifact, ArtifactId, ArtifactSource, CancellationToken, ErrorCode, ErrorKind, GrapheneError,
    OperationController, Progress, Result, Sha1Digest, Sha256Digest,
};
use reqwest::{Client, StatusCode};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::{
    fs::File as StdFile,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
    sync::{OwnedSemaphorePermit, Semaphore},
    time::sleep,
};
use tracing::{debug, warn};

/// Completed streamed transfer data. The body itself remains disk-backed at the caller-provided
/// temporary path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferResult {
    pub path: PathBuf,
    pub bytes: u64,
    pub sha1: Sha1Digest,
    pub sha256: Sha256Digest,
    pub source_host: Option<String>,
}

/// Verification result for an existing disk-backed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedFile {
    pub bytes: u64,
    pub sha1: Sha1Digest,
    pub sha256: Sha256Digest,
}

/// Shared provider-neutral HTTP client with bounded concurrency.
#[derive(Clone)]
pub struct NetworkClient {
    client: Client,
    config: Arc<NetworkConfig>,
    downloads: Arc<Semaphore>,
}

impl std::fmt::Debug for NetworkClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl NetworkClient {
    /// Builds the shared HTTP client. TLS certificate validation remains enabled; there is no
    /// ordinary configuration switch to disable it.
    pub fn new(config: NetworkConfig) -> Result<Self> {
        config.validate()?;
        let mut builder = Client::builder()
            .connect_timeout(config.connect_timeout)
            .timeout(config.request_timeout)
            .redirect(reqwest::redirect::Policy::limited(
                config.redirect_policy.max_redirects,
            ))
            .user_agent(config.user_agent.clone());

        builder = match &config.proxy {
            ProxyPolicy::System => builder,
            ProxyPolicy::None => builder.no_proxy(),
            ProxyPolicy::Explicit(_) => {
                let proxy = reqwest::Proxy::all(
                    config
                        .proxy
                        .explicit_url()
                        .expect("explicit proxy variant has URL"),
                )
                .map_err(|_source| {
                    GrapheneError::new(
                        ErrorCode::NetworkProxyInvalid,
                        ErrorKind::Configuration,
                        "explicit proxy configuration is invalid",
                    )
                })?;
                builder.proxy(proxy)
            }
        };

        let client = builder.build().map_err(|source| {
            GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                "failed to construct shared HTTP client",
            )
            .with_source(source)
        })?;
        Ok(Self {
            downloads: Arc::new(Semaphore::new(config.max_concurrent_downloads)),
            client,
            config: Arc::new(config),
        })
    }

    /// Returns immutable Graphene-owned network policy.
    #[must_use]
    pub fn config(&self) -> &NetworkConfig {
        &self.config
    }

    /// Streams one artifact through ordered source fallback into `temporary_path`, verifying size
    /// and hashes before returning success.
    pub async fn download_to(
        &self,
        artifact: &Artifact,
        temporary_path: &Path,
        operation: &OperationController,
    ) -> Result<TransferResult> {
        if artifact.sources.is_empty() {
            return Err(GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                "artifact has no acquisition sources",
            ));
        }
        if !artifact.integrity.is_verifiable() {
            return Err(GrapheneError::new(
                ErrorCode::CacheIdentityUnavailable,
                ErrorKind::Integrity,
                "Phase 0 verified cache acquisition requires SHA-1 or SHA-256",
            ));
        }
        checkpoint(operation, ErrorCode::DownloadCancelled)?;
        let _permit = self.acquire_download_permit(operation).await?;

        let sources = ordered_sources(artifact);

        let mut evidence = Vec::new();
        let mut last_error = None;
        for (source_index, source) in sources {
            checkpoint(operation, ErrorCode::DownloadCancelled)?;
            let host = safe_host(source.url());
            match self
                .download_source(artifact, source, temporary_path, operation)
                .await
            {
                Ok(result) => return Ok(result),
                Err(error) => {
                    evidence.push(format!(
                        "source={source_index} host={} code={}",
                        host.as_deref().unwrap_or("unknown"),
                        error.code
                    ));
                    warn!(
                        operation_id = %operation.handle().id(),
                        artifact_id = %artifact.id,
                        source_host = host.as_deref().unwrap_or("unknown"),
                        code = %error.code,
                        "artifact source failed; considering fallback"
                    );
                    last_error = Some(error);
                }
            }
        }

        let mut error = last_error.unwrap_or_else(|| {
            GrapheneError::new(
                ErrorCode::NetworkRequestFailed,
                ErrorKind::Network,
                "all artifact sources failed",
            )
        });
        if !evidence.is_empty() {
            error = error.with_context("source_attempts", evidence.join("; "));
        }
        Err(error)
    }

    async fn acquire_download_permit(
        &self,
        operation: &OperationController,
    ) -> Result<OwnedSemaphorePermit> {
        let token = operation.cancellation_token();
        tokio::select! {
            () = token.cancelled() => Err(cancelled_error(ErrorCode::DownloadCancelled)),
            permit = Arc::clone(&self.downloads).acquire_owned() => permit.map_err(|_| {
                GrapheneError::new(
                    ErrorCode::InternalInvariantViolation,
                    ErrorKind::Internal,
                    "download concurrency semaphore was unexpectedly closed",
                )
            }),
        }
    }

    async fn download_source(
        &self,
        artifact: &Artifact,
        source: &ArtifactSource,
        temporary_path: &Path,
        operation: &OperationController,
    ) -> Result<TransferResult> {
        let host = safe_host(source.url());
        let mut last_error = None;
        for attempt in 1..=self.config.retry_policy.max_attempts {
            checkpoint(operation, ErrorCode::DownloadCancelled)?;
            if attempt > 1 {
                let delay = jittered_delay(
                    self.config.retry_policy.delay_before_attempt(attempt),
                    self.config.retry_policy.max_delay,
                    artifact.id,
                    attempt,
                );
                debug!(
                    operation_id = %operation.handle().id(),
                    artifact_id = %artifact.id,
                    attempt,
                    source_host = host.as_deref().unwrap_or("unknown"),
                    ?delay,
                    "retrying artifact source"
                );
                sleep_with_cancellation(delay, operation).await?;
            }

            debug!(
                operation_id = %operation.handle().id(),
                artifact_id = %artifact.id,
                attempt,
                source_host = host.as_deref().unwrap_or("unknown"),
                "starting artifact download attempt"
            );
            match self
                .download_once(artifact, source, temporary_path, operation)
                .await
            {
                Ok(transfer) => return Ok(transfer),
                Err(attempt_error) => {
                    let retry = attempt_error.retryable
                        && attempt < self.config.retry_policy.max_attempts
                        && !operation.is_cancelled();
                    last_error = Some(attempt_error.error);
                    if !retry {
                        break;
                    }
                }
            }
        }
        Err(last_error.unwrap_or_else(|| {
            GrapheneError::new(
                ErrorCode::NetworkRequestFailed,
                ErrorKind::Network,
                "artifact source failed without an error result",
            )
        }))
    }

    async fn download_once(
        &self,
        artifact: &Artifact,
        source: &ArtifactSource,
        temporary_path: &Path,
        operation: &OperationController,
    ) -> std::result::Result<TransferResult, AttemptError> {
        checkpoint(operation, ErrorCode::DownloadCancelled).map_err(AttemptError::non_retryable)?;
        operation
            .set_stage("download")
            .map_err(AttemptError::non_retryable)?;
        let url = reqwest::Url::parse(source.url()).map_err(|source_error| {
            AttemptError::non_retryable(
                GrapheneError::new(
                    ErrorCode::ConfigInvalid,
                    ErrorKind::Configuration,
                    "artifact source URL is invalid",
                )
                .with_source(source_error),
            )
        })?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(AttemptError::non_retryable(GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                "artifact source URL must use HTTP or HTTPS",
            )));
        }
        let host = url.host_str().map(str::to_owned);

        let token = operation.cancellation_token();
        let response = tokio::select! {
            () = token.cancelled() => return Err(AttemptError::non_retryable(cancelled_error(ErrorCode::DownloadCancelled))),
            response = self.client.get(url).send() => response.map_err(classify_reqwest_error)?,
        };
        checkpoint(operation, ErrorCode::DownloadCancelled).map_err(AttemptError::non_retryable)?;

        let status = response.status();
        if !status.is_success() {
            return Err(classify_status(status));
        }

        let response_length = response.content_length();
        match (artifact.expected_size, response_length) {
            (Some(expected), Some(actual)) if expected != actual => {
                return Err(AttemptError::non_retryable(size_mismatch_error(
                    expected, actual,
                )));
            }
            _ => {}
        }
        let total = artifact.expected_size.or(response_length);
        operation
            .set_progress(Progress::Bytes {
                completed: 0,
                total,
            })
            .map_err(AttemptError::non_retryable)?;

        if let Some(parent) = temporary_path.parent() {
            fs::create_dir_all(parent).await.map_err(|source_error| {
                AttemptError::non_retryable(
                    GrapheneError::new(
                        ErrorCode::DirectoryCreateFailed,
                        ErrorKind::Filesystem,
                        "failed to create temporary download directory",
                    )
                    .with_source(source_error),
                )
            })?;
        }
        // A retry or source fallback reuses this operation-owned temp pathname. Remove only that
        // managed incomplete file before using create_new, so stale partial bytes can never be
        // appended to or mistaken for a fresh attempt.
        match fs::remove_file(temporary_path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source_error) => {
                return Err(AttemptError::non_retryable(
                    GrapheneError::new(
                        ErrorCode::FileWriteFailed,
                        ErrorKind::Filesystem,
                        "failed to clear incomplete temporary download",
                    )
                    .with_source(source_error),
                ));
            }
        }

        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(temporary_path)
            .await
            .map_err(|source_error| {
                AttemptError::non_retryable(
                    GrapheneError::new(
                        ErrorCode::FileOpenFailed,
                        ErrorKind::Filesystem,
                        "failed to create temporary download file",
                    )
                    .with_source(source_error),
                )
            })?;
        let mut sha1 = Sha1::new();
        let mut sha256 = Sha256::new();
        let mut bytes = 0_u64;
        let mut stream = response.bytes_stream();

        loop {
            let token = operation.cancellation_token();
            let chunk = tokio::select! {
                () = token.cancelled() => return Err(AttemptError::non_retryable(cancelled_error(ErrorCode::DownloadCancelled))),
                chunk = stream.next() => chunk,
            };
            let Some(chunk) = chunk else { break };
            let chunk = chunk.map_err(classify_response_body_error)?;

            file.write_all(&chunk).await.map_err(|source_error| {
                AttemptError::non_retryable(
                    GrapheneError::new(
                        ErrorCode::FileWriteFailed,
                        ErrorKind::Filesystem,
                        "failed while streaming artifact to disk",
                    )
                    .with_source(source_error),
                )
            })?;

            sha1.update(&chunk);
            sha256.update(&chunk);
            bytes = bytes.saturating_add(chunk.len() as u64);

            match artifact.expected_size {
                Some(expected) if bytes > expected => {
                    return Err(AttemptError::non_retryable(size_mismatch_error(
                        expected, bytes,
                    )));
                }
                _ => {}
            }

            operation
                .set_progress(Progress::Bytes {
                    completed: bytes,
                    total,
                })
                .map_err(AttemptError::non_retryable)?;
        }

        file.flush().await.map_err(|source_error| {
            AttemptError::non_retryable(
                GrapheneError::new(
                    ErrorCode::FileWriteFailed,
                    ErrorKind::Filesystem,
                    "failed to flush temporary artifact",
                )
                .with_source(source_error),
            )
        })?;

        file.sync_all().await.map_err(|source_error| {
            AttemptError::non_retryable(
                GrapheneError::new(
                    ErrorCode::FileWriteFailed,
                    ErrorKind::Filesystem,
                    "failed to synchronize temporary artifact",
                )
                .with_source(source_error),
            )
        })?;

        drop(file);

        checkpoint(operation, ErrorCode::DownloadCancelled).map_err(AttemptError::non_retryable)?;
        let sha1 = sha1.finalize();
        let sha256 = sha256.finalize();
        let mut sha1_bytes = [0_u8; 20];
        let mut sha256_bytes = [0_u8; 32];

        sha1_bytes.copy_from_slice(&sha1);
        sha256_bytes.copy_from_slice(&sha256);

        let transfer = TransferResult {
            path: temporary_path.to_path_buf(),
            bytes,
            sha1: Sha1Digest::from_bytes(sha1_bytes),
            sha256: Sha256Digest::from_bytes(sha256_bytes),
            source_host: host,
        };

        debug!(
            operation_id = %operation.handle().id(),
            artifact_id = %artifact.id,
            bytes,
            source_host = transfer.source_host.as_deref().unwrap_or("unknown"),
            "artifact response streamed to temporary storage"
        );

        operation
            .set_stage("verify")
            .map_err(AttemptError::non_retryable)?;
        checkpoint(operation, ErrorCode::DownloadCancelled).map_err(AttemptError::non_retryable)?;
        verify_transfer(artifact, &transfer).map_err(|error| AttemptError {
            error,
            retryable: false,
        })?;

        debug!(
            operation_id = %operation.handle().id(),
            artifact_id = %artifact.id,
            bytes,
            "artifact transfer integrity verified"
        );
        Ok(transfer)
    }

    /// Incrementally verifies an existing cache file without buffering the full object in memory.
    /// Cancellation is checked between hashing chunks.
    pub async fn verify_file(
        &self,
        path: &Path,
        artifact: &Artifact,
        cancellation: &CancellationToken,
    ) -> Result<VerifiedFile> {
        let path = path.to_path_buf();
        let expected_size = artifact.expected_size;
        let integrity = artifact.integrity.clone();
        let cancellation = cancellation.clone();

        tokio::task::spawn_blocking(move || {
            verify_file_sync(&path, expected_size, &integrity, &cancellation)
        })
        .await
        .map_err(|source| {
            GrapheneError::new(
                ErrorCode::InternalInvariantViolation,
                ErrorKind::Internal,
                "cache verification worker failed",
            )
            .with_source(source)
        })?
    }
}

#[derive(Debug)]
struct AttemptError {
    error: GrapheneError,
    retryable: bool,
}

impl AttemptError {
    fn non_retryable(error: GrapheneError) -> Self {
        Self {
            error,
            retryable: false,
        }
    }
}

fn ordered_sources(artifact: &Artifact) -> Vec<(usize, &ArtifactSource)> {
    let mut sources = artifact.sources.iter().enumerate().collect::<Vec<_>>();
    sources.sort_by_key(|(index, source)| (source.priority(), *index));
    sources
}

fn size_mismatch_error(expected: u64, actual: u64) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::DownloadSizeMismatch,
        ErrorKind::Integrity,
        "downloaded artifact size does not match expectation",
    )
    .with_context("expected", expected.to_string())
    .with_context("actual", actual.to_string())
}

fn classify_status(status: StatusCode) -> AttemptError {
    AttemptError {
        retryable: retryable_status(status),
        error: GrapheneError::new(
            ErrorCode::NetworkStatusError,
            ErrorKind::Network,
            "artifact source returned an unsuccessful HTTP status",
        )
        .with_context("status", status.as_u16().to_string()),
    }
}

fn classify_reqwest_error(source: reqwest::Error) -> AttemptError {
    if source.is_timeout() {
        return AttemptError {
            error: GrapheneError::new(
                ErrorCode::NetworkTimeout,
                ErrorKind::Timeout,
                "network request timed out",
            ),
            retryable: true,
        };
    }

    if source.is_redirect() {
        return AttemptError::non_retryable(GrapheneError::new(
            ErrorCode::NetworkRedirectRejected,
            ErrorKind::Network,
            "network redirect policy rejected the request",
        ));
    }

    let retryable = source.is_connect() || source.is_request() || source.is_body();
    AttemptError {
        error: GrapheneError::new(
            ErrorCode::NetworkRequestFailed,
            ErrorKind::Network,
            "network request failed",
        ),
        retryable,
    }
}

fn classify_response_body_error(source: reqwest::Error) -> AttemptError {
    if source.is_timeout() {
        return AttemptError {
            error: GrapheneError::new(
                ErrorCode::NetworkTimeout,
                ErrorKind::Timeout,
                "network response body timed out",
            ),
            retryable: true,
        };
    }

    // At this point a successful HTTP response has already started streaming. Any reqwest error
    // produced by the response body means the transfer was interrupted or otherwise incomplete.
    // Treat it as transient and let the bounded retry policy decide whether another attempt is
    // allowed. Integrity and size failures are classified separately after streaming and remain
    // non-retryable.
    AttemptError {
        error: GrapheneError::new(
            ErrorCode::NetworkRequestFailed,
            ErrorKind::Network,
            "network response body failed while streaming",
        ),
        retryable: true,
    }
}

fn verify_file_sync(
    path: &Path,
    expected_size: Option<u64>,
    integrity: &graphene_core::ArtifactIntegrity,
    cancellation: &CancellationToken,
) -> Result<VerifiedFile> {
    let mut file = StdFile::open(path).map_err(|source| {
        GrapheneError::new(
            ErrorCode::FileOpenFailed,
            ErrorKind::Filesystem,
            "failed to open cached artifact for verification",
        )
        .with_source(source)
    })?;

    let mut sha1 = Sha1::new();
    let mut sha256 = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        if cancellation.is_cancelled() {
            return Err(cancelled_error(ErrorCode::DownloadCancelled));
        }

        let read = file.read(&mut buffer).map_err(|source| {
            GrapheneError::new(
                ErrorCode::FileOpenFailed,
                ErrorKind::Filesystem,
                "failed while reading cached artifact for verification",
            )
            .with_source(source)
        })?;

        if read == 0 {
            break;
        }

        sha1.update(&buffer[..read]);
        sha256.update(&buffer[..read]);
        bytes = bytes.saturating_add(read as u64);
    }

    let sha1 = sha1.finalize();
    let sha256 = sha256.finalize();
    let mut sha1_bytes = [0_u8; 20];
    let mut sha256_bytes = [0_u8; 32];

    sha1_bytes.copy_from_slice(&sha1);
    sha256_bytes.copy_from_slice(&sha256);

    let verified = VerifiedFile {
        bytes,
        sha1: Sha1Digest::from_bytes(sha1_bytes),
        sha256: Sha256Digest::from_bytes(sha256_bytes),
    };
    match expected_size {
        Some(expected) if verified.bytes != expected => {
            return Err(GrapheneError::new(
                ErrorCode::DownloadSizeMismatch,
                ErrorKind::Integrity,
                "cached artifact size does not match expectation",
            )
            .with_context("expected", expected.to_string())
            .with_context("actual", verified.bytes.to_string()));
        }
        _ => {}
    }

    match integrity.sha1() {
        Some(expected) if verified.sha1 != expected => {
            return Err(GrapheneError::new(
                ErrorCode::HashMismatch,
                ErrorKind::Integrity,
                "cached artifact SHA-1 does not match expectation",
            )
            .with_context("algorithm", "sha1"));
        }
        _ => {}
    }

    match integrity.sha256() {
        Some(expected) if verified.sha256 != expected => {
            return Err(GrapheneError::new(
                ErrorCode::HashMismatch,
                ErrorKind::Integrity,
                "cached artifact SHA-256 does not match expectation",
            )
            .with_context("algorithm", "sha256"));
        }
        _ => {}
    }
    Ok(verified)
}

fn checkpoint(operation: &OperationController, code: ErrorCode) -> Result<()> {
    if operation.is_cancelled() {
        Err(cancelled_error(code))
    } else {
        Ok(())
    }
}

fn cancelled_error(code: ErrorCode) -> GrapheneError {
    GrapheneError::new(code, ErrorKind::Cancelled, "download was cancelled")
}

fn safe_host(url: &str) -> Option<String> {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
}

fn jittered_delay(
    base: Duration,
    maximum: Duration,
    artifact_id: ArtifactId,
    attempt: usize,
) -> Duration {
    let mut seed = attempt as u64;
    for byte in artifact_id.to_string().bytes() {
        seed = seed
            .wrapping_mul(1_099_511_628_211)
            .wrapping_add(u64::from(byte));
    }

    let base_nanos = base.as_nanos();
    let spread = (base_nanos / 8).max(1);
    let bucket_count = spread.saturating_mul(2).saturating_add(1);
    let bucket = u128::from(seed) % bucket_count;
    let jittered = if bucket >= spread {
        base_nanos.saturating_add(bucket - spread)
    } else {
        base_nanos.saturating_sub(spread - bucket)
    };
    let bounded = jittered.max(1).min(maximum.as_nanos());
    Duration::from_nanos(bounded.min(u128::from(u64::MAX)) as u64)
}

async fn sleep_with_cancellation(delay: Duration, operation: &OperationController) -> Result<()> {
    let token = operation.cancellation_token();
    tokio::select! {
        () = token.cancelled() => Err(cancelled_error(ErrorCode::DownloadCancelled)),
        () = sleep(delay) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphene_core::ArtifactIntegrity;

    #[test]
    fn source_order_is_priority_then_declaration_order() {
        let artifact = Artifact::new(
            vec![
                ArtifactSource::new("https://one.invalid").with_priority(5),
                ArtifactSource::new("https://two.invalid").with_priority(1),
                ArtifactSource::new("https://three.invalid").with_priority(1),
            ],
            ArtifactIntegrity::none(),
        );
        let ordered = ordered_sources(&artifact)
            .into_iter()
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(ordered, vec![1, 2, 0]);
    }

    #[test]
    fn status_mapping_distinguishes_retryability() {
        let transient = classify_status(StatusCode::SERVICE_UNAVAILABLE);
        assert!(transient.retryable);
        assert_eq!(transient.error.code, ErrorCode::NetworkStatusError);

        let permanent = classify_status(StatusCode::NOT_FOUND);
        assert!(!permanent.retryable);
        assert_eq!(permanent.error.code, ErrorCode::NetworkStatusError);
    }

    #[test]
    fn jitter_stays_positive_and_bounded() {
        let id = ArtifactId::new();
        let base = Duration::from_millis(100);
        let maximum = Duration::from_millis(110);

        for attempt in 2..=10 {
            let delay = jittered_delay(base, maximum, id, attempt);
            assert!(!delay.is_zero());
            assert!(delay <= maximum);
        }
    }
}
