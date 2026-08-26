use super::{NetworkClient, cancelled_error, checkpoint};
use futures_util::StreamExt;
use graphene_core::{
    ErrorCode, ErrorKind, GrapheneError, OperationController, Progress, Result, Sha256Digest,
    Sha512Digest,
};
use sha2::{Digest, Sha256, Sha512};
use std::path::Path;
use tokio::{fs, io::AsyncWriteExt};

/// Result of an integrity-unobserved source transfer: bytes plus observed digests only.
///
/// This path is exclusively for pack-source snapshotting where the caller supplied no trustworthy
/// predeclared integrity. The observed SHA-256 becomes the immutable cache identity afterwards; it
/// is never treated as publisher authenticity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedTransfer {
    pub bytes: u64,
    pub sha256: Sha256Digest,
    pub sha512: Sha512Digest,
}

impl NetworkClient {
    /// Streams one HTTPS URL to `temporary_path` under a hard byte limit while computing observed
    /// digests. No predeclared integrity exists on this path by definition; scheme/userinfo and
    /// redirect-downgrade policies still apply.
    pub async fn download_observed_to(
        &self,
        raw_url: &str,
        temporary_path: &Path,
        max_bytes: u64,
        operation: &OperationController,
    ) -> Result<ObservedTransfer> {
        let _permit = self.acquire_download_permit(operation).await?;
        checkpoint(operation, ErrorCode::DownloadCancelled)?;

        let url = reqwest::Url::parse(raw_url).map_err(|source| {
            GrapheneError::new(
                ErrorCode::PackSourceInvalid,
                ErrorKind::Network,
                "pack source URL is invalid",
            )
            .with_source(source)
        })?;
        if url.scheme() != "https" {
            return Err(GrapheneError::new(
                ErrorCode::PackSourceInvalid,
                ErrorKind::Network,
                "pack source must use HTTPS",
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(GrapheneError::new(
                ErrorCode::PackSourceInvalid,
                ErrorKind::Network,
                "pack source URL must not carry userinfo",
            ));
        }

        let token = operation.cancellation_token();
        let response = tokio::select! {
            () = token.cancelled() => return Err(cancelled_error(ErrorCode::DownloadCancelled)),
            response = self.client.get(url).send() => response.map_err(|source| {
                GrapheneError::new(
                    ErrorCode::NetworkRequestFailed,
                    ErrorKind::Network,
                    "pack source request failed",
                )
                .with_source(source)
            }),
        }?;

        // HTTPS downgrade through redirects is rejected exactly like verified artifact transfers.
        if response.url().scheme() != "https" {
            return Err(GrapheneError::new(
                ErrorCode::NetworkRedirectRejected,
                ErrorKind::Network,
                "pack source redirect attempted to downgrade HTTPS",
            ));
        }
        let final_url = response.url().clone();
        if !final_url.username().is_empty() || final_url.password().is_some() {
            return Err(GrapheneError::new(
                ErrorCode::PackSourceInvalid,
                ErrorKind::Network,
                "pack source redirect carried userinfo",
            ));
        }

        let status = response.status();
        if !status.is_success() {
            return Err(GrapheneError::new(
                ErrorCode::NetworkStatusError,
                ErrorKind::Network,
                "pack source returned an unsuccessful HTTP status",
            )
            .with_context("status", status.as_u16().to_string()));
        }

        operation.set_stage("snapshot-download")?;
        operation.set_progress(Progress::Bytes {
            completed: 0,
            total: response.content_length(),
        })?;

        if let Some(parent) = temporary_path.parent() {
            fs::create_dir_all(parent).await.map_err(|source| {
                GrapheneError::new(
                    ErrorCode::DirectoryCreateFailed,
                    ErrorKind::Filesystem,
                    "failed to create pack source temporary directory",
                )
                .with_source(source)
            })?;
        }
        match fs::remove_file(temporary_path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(GrapheneError::new(
                    ErrorCode::FileWriteFailed,
                    ErrorKind::Filesystem,
                    "failed to clear incomplete pack source temporary file",
                )
                .with_source(source));
            }
        }

        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(temporary_path)
            .await
            .map_err(|source| {
                GrapheneError::new(
                    ErrorCode::FileOpenFailed,
                    ErrorKind::Filesystem,
                    "failed to create pack source temporary file",
                )
                .with_source(source)
            })?;

        let mut sha256 = Sha256::new();
        let mut sha512 = Sha512::new();
        let mut bytes = 0_u64;
        let mut stream = response.bytes_stream();

        loop {
            let token = operation.cancellation_token();
            let chunk = tokio::select! {
                () = token.cancelled() => return Err(cancelled_error(ErrorCode::DownloadCancelled)),
                chunk = stream.next() => chunk,
            };
            let Some(chunk) = chunk else { break };
            let chunk = chunk.map_err(|source| {
                GrapheneError::new(
                    ErrorCode::NetworkRequestFailed,
                    ErrorKind::Network,
                    "pack source body failed while streaming",
                )
                .with_source(source)
            })?;

            file.write_all(&chunk).await.map_err(|source| {
                GrapheneError::new(
                    ErrorCode::FileWriteFailed,
                    ErrorKind::Filesystem,
                    "failed while streaming pack source to disk",
                )
                .with_source(source)
            })?;
            sha256.update(&chunk);
            sha512.update(&chunk);
            bytes = bytes.saturating_add(chunk.len() as u64);
            if bytes > max_bytes {
                return Err(GrapheneError::new(
                    ErrorCode::PackSourceTooLarge,
                    ErrorKind::Modpack,
                    "pack source exceeds its byte limit",
                ));
            }

            operation.set_progress(Progress::Bytes {
                completed: bytes,
                total: None,
            })?;
        }

        file.flush().await.map_err(|source| {
            GrapheneError::new(
                ErrorCode::FileWriteFailed,
                ErrorKind::Filesystem,
                "failed to flush pack source temporary file",
            )
            .with_source(source)
        })?;
        file.sync_all().await.map_err(|source| {
            GrapheneError::new(
                ErrorCode::FileWriteFailed,
                ErrorKind::Filesystem,
                "failed to synchronize pack source temporary file",
            )
            .with_source(source)
        })?;
        drop(file);

        checkpoint(operation, ErrorCode::DownloadCancelled)?;
        let sha256_bytes: [u8; 32] = sha256.finalize().into();
        let sha512_bytes: [u8; 64] = sha512.finalize().into();

        Ok(ObservedTransfer {
            bytes,
            sha256: Sha256Digest::from_bytes(sha256_bytes),
            sha512: Sha512Digest::from_bytes(sha512_bytes),
        })
    }
}
