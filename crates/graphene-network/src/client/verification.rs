use super::{NetworkClient, VerifiedFile, cancelled_error};
use graphene_core::{
    Artifact, ArtifactIntegrity, CancellationToken, ErrorCode, ErrorKind, GrapheneError, Result,
    Sha1Digest, Sha256Digest,
};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::{fs::File as StdFile, io::Read, path::Path};

impl NetworkClient {
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

fn verify_file_sync(
    path: &Path,
    expected_size: Option<u64>,
    integrity: &ArtifactIntegrity,
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
