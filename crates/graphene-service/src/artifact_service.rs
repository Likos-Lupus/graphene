use crate::context::ServiceContext;
use graphene_core::{
    Artifact, CachePolicy, ErrorCode, ErrorKind, GrapheneError, OperationController,
    OperationHandle, OperationResult, Progress,
};
use graphene_network::{TransferResult, VerifiedFile};
use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};
use tracing::{debug, info};

/// Indicates whether a verified result came from an existing cache object or a new transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadDisposition {
    CacheHit,
    Downloaded,
}

/// Stable Graphene-owned result of artifact acquisition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedArtifact {
    pub artifact_id: graphene_core::ArtifactId,
    pub path: PathBuf,
    pub bytes: u64,
    pub sha1: graphene_core::Sha1Digest,
    pub sha256: graphene_core::Sha256Digest,
    pub sha512: graphene_core::Sha512Digest,
    pub disposition: DownloadDisposition,
}

/// Generic verified artifact service composed from network and storage foundations.
#[derive(Clone)]
pub struct ArtifactService {
    context: Arc<ServiceContext>,
}

impl std::fmt::Debug for ArtifactService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtifactService").finish_non_exhaustive()
    }
}

impl ArtifactService {
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        Self { context }
    }

    /// Prepares a cancellable child acquisition operation and returns immediately with its handle
    /// and opaque future. Work starts when [`ArtifactOperation::await_result`] is polled.
    #[must_use]
    pub fn acquire(
        &self,
        artifact: Artifact,
        parent: Option<&OperationHandle>,
    ) -> ArtifactOperation {
        let controller = parent.map_or_else(
            || self.context.operations.create("artifact-acquire"),
            |parent| {
                self.context
                    .operations
                    .create_child(parent, "artifact-acquire")
            },
        );
        let operation = controller.handle();
        debug!(
            operation_id = %operation.id(),
            artifact_id = %artifact.id,
            parent_id = ?operation.parent_id(),
            "artifact operation created"
        );
        let service = self.clone();
        let future = Box::pin(async move { service.run_acquire(artifact, controller).await });
        ArtifactOperation { operation, future }
    }

    async fn run_acquire(
        &self,
        artifact: Artifact,
        controller: OperationController,
    ) -> graphene_core::Result<VerifiedArtifact> {
        if let Err(error) = controller.start() {
            let _ = controller.cancelled();
            return Err(error);
        }

        let result = self.acquire_inner(&artifact, &controller).await;
        match result {
            Ok(value) => match controller.succeed()? {
                OperationResult::Succeeded => {
                    debug!(
                        operation_id = %controller.handle().id(),
                        artifact_id = %artifact.id,
                        state = "Succeeded",
                        "artifact operation reached terminal state"
                    );
                    Ok(value)
                }

                OperationResult::Cancelled => {
                    debug!(
                        operation_id = %controller.handle().id(),
                        artifact_id = %artifact.id,
                        state = "Cancelled",
                        "artifact operation reached terminal state"
                    );
                    Err(cancelled_error())
                }

                OperationResult::Failed { .. } => Err(GrapheneError::new(
                    ErrorCode::InternalInvariantViolation,
                    ErrorKind::Internal,
                    "artifact operation failed during success transition",
                )),
            },

            Err(error) if error.is_cancelled() || controller.is_cancelled() => {
                let returned = if error.is_cancelled() {
                    error
                } else {
                    cancelled_error()
                };
                let _ = controller.cancelled();
                debug!(
                    operation_id = %controller.handle().id(),
                    artifact_id = %artifact.id,
                    state = "Cancelled",
                    "artifact operation reached terminal state"
                );
                Err(returned)
            }

            Err(error) => {
                let _ = controller.fail(error.summary());
                debug!(
                    operation_id = %controller.handle().id(),
                    artifact_id = %artifact.id,
                    state = "Failed",
                    code = %error.code,
                    "artifact operation reached terminal state"
                );

                Err(error)
            }
        }
    }

    async fn acquire_inner(
        &self,
        artifact: &Artifact,
        controller: &OperationController,
    ) -> graphene_core::Result<VerifiedArtifact> {
        // Cache identity is derived from declared integrity before any source handling, so a
        // verifiable cache-only artifact (sources = []) can be served entirely from cache.
        let cache = self.context.storage.cache_address(&artifact.integrity)?;
        let cache_path = cache.path().to_path_buf();

        controller.set_stage("cache-lookup")?;
        let initial_hit = if artifact.cache_policy == CachePolicy::UseVerified {
            self.try_cache_hit(&cache_path, artifact, controller)
                .await?
        } else {
            None
        };

        if let Some(hit) = initial_hit {
            controller.set_progress(Progress::Bytes {
                completed: hit.bytes,
                total: artifact.expected_size.or(Some(hit.bytes)),
            })?;
            info!(
                operation_id = %controller.handle().id(),
                artifact_id = %artifact.id,
                "verified artifact cache hit"
            );

            return Ok(from_verified_file(
                artifact,
                cache_path,
                hit,
                DownloadDisposition::CacheHit,
            ));
        }

        if artifact.sources.is_empty() {
            return Err(GrapheneError::new(
                ErrorCode::ArtifactSourceUnavailable,
                ErrorKind::Integrity,
                "artifact has no valid cached object and no acquisition sources",
            ));
        }

        let gate = self.context.artifact_gate(&cache_path);
        let token = controller.cancellation_token();
        let guard = tokio::select! {
            () = token.cancelled() => return Err(cancelled_error()),
            guard = gate.lock() => guard,
        };

        let gated_hit = if artifact.cache_policy == CachePolicy::UseVerified {
            self.try_cache_hit(&cache_path, artifact, controller)
                .await?
        } else {
            None
        };

        if let Some(hit) = gated_hit {
            drop(guard);
            controller.set_progress(Progress::Bytes {
                completed: hit.bytes,
                total: artifact.expected_size.or(Some(hit.bytes)),
            })?;

            return Ok(from_verified_file(
                artifact,
                cache_path,
                hit,
                DownloadDisposition::CacheHit,
            ));
        }

        checkpoint(controller)?;
        let temporary = self
            .context
            .storage
            .download_temp_path(artifact.id, controller.handle().id())?;
        let mut temp_guard = TempGuard::new(temporary.clone());
        let transfer = self
            .context
            .network
            .download_to(artifact, &temporary, controller)
            .await?;

        // Publish an explicit pre-commit stage and yield once so observers can request cancellation
        // before the documented point of no return. Cancellation still wins only if requested
        // before the subsequent seal.
        controller.set_stage("pre-commit")?;
        tokio::task::yield_now().await;
        checkpoint(controller)?;
        if !controller.seal_cancellation() {
            return Err(cancelled_error());
        }

        controller.set_stage("commit")?;
        self.context
            .storage
            .commit_verified(&temporary, &cache_path)?;
        temp_guard.committed = true;

        drop(guard);
        debug!(
            operation_id = %controller.handle().id(),
            artifact_id = %artifact.id,
            bytes = transfer.bytes,
            "verified artifact committed to cache"
        );

        Ok(from_transfer(
            artifact,
            cache_path,
            transfer,
            DownloadDisposition::Downloaded,
        ))
    }

    async fn try_cache_hit(
        &self,
        cache_path: &Path,
        artifact: &Artifact,
        controller: &OperationController,
    ) -> graphene_core::Result<Option<VerifiedFile>> {
        if !self.context.storage.committed_file_exists(cache_path)? {
            return Ok(None);
        }

        let token = controller.cancellation_token();
        match self
            .context
            .network
            .verify_file(cache_path, artifact, &token)
            .await
        {
            Ok(verified) => Ok(Some(verified)),
            Err(error)
                if matches!(
                    error.code,
                    ErrorCode::HashMismatch | ErrorCode::DownloadSizeMismatch
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

/// Cancellable prepared artifact acquisition.
pub struct ArtifactOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = graphene_core::Result<VerifiedArtifact>> + Send + 'static>>,
}

impl std::fmt::Debug for ArtifactOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtifactOperation")
            .field("operation", &self.operation)
            .finish_non_exhaustive()
    }
}

impl ArtifactOperation {
    /// Returns the observer/cancellation handle before awaiting the transfer.
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }

    /// Runs and awaits the complete stream -> verify -> safe cache commit pipeline.
    pub async fn await_result(self) -> graphene_core::Result<VerifiedArtifact> {
        self.future.await
    }
}

struct TempGuard {
    path: PathBuf,
    committed: bool,
}

impl TempGuard {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            committed: false,
        }
    }
}

impl Drop for TempGuard {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn checkpoint(controller: &OperationController) -> graphene_core::Result<()> {
    if controller.is_cancelled() {
        Err(cancelled_error())
    } else {
        Ok(())
    }
}

fn cancelled_error() -> GrapheneError {
    GrapheneError::new(
        ErrorCode::DownloadCancelled,
        ErrorKind::Cancelled,
        "artifact acquisition was cancelled",
    )
}

fn from_verified_file(
    artifact: &Artifact,
    path: PathBuf,
    verified: VerifiedFile,
    disposition: DownloadDisposition,
) -> VerifiedArtifact {
    VerifiedArtifact {
        artifact_id: artifact.id,
        path,
        bytes: verified.bytes,
        sha1: verified.sha1,
        sha256: verified.sha256,
        sha512: verified.sha512,
        disposition,
    }
}

fn from_transfer(
    artifact: &Artifact,
    path: PathBuf,
    transfer: TransferResult,
    disposition: DownloadDisposition,
) -> VerifiedArtifact {
    VerifiedArtifact {
        artifact_id: artifact.id,
        path,
        bytes: transfer.bytes,
        sha1: transfer.sha1,
        sha256: transfer.sha256,
        sha512: transfer.sha512,
        disposition,
    }
}
