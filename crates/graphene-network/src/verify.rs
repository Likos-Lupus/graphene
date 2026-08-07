use crate::TransferResult;
use graphene_core::{Artifact, ErrorCode, ErrorKind, GrapheneError, Result};

/// Verifies expected size and every declared digest before a transfer may be committed.
pub fn verify_transfer(artifact: &Artifact, transfer: &TransferResult) -> Result<()> {
    match artifact.expected_size {
        Some(expected) if transfer.bytes != expected => {
            return Err(GrapheneError::new(
                ErrorCode::DownloadSizeMismatch,
                ErrorKind::Integrity,
                "downloaded artifact size does not match expectation",
            )
            .with_context("expected", expected.to_string())
            .with_context("actual", transfer.bytes.to_string()));
        }
        _ => {}
    }

    match artifact.integrity.sha1() {
        Some(expected) if transfer.sha1 != expected => {
            return Err(GrapheneError::new(
                ErrorCode::HashMismatch,
                ErrorKind::Integrity,
                "downloaded artifact SHA-1 does not match expectation",
            )
            .with_context("algorithm", "sha1"));
        }
        _ => {}
    }

    match artifact.integrity.sha256() {
        Some(expected) if transfer.sha256 != expected => {
            return Err(GrapheneError::new(
                ErrorCode::HashMismatch,
                ErrorKind::Integrity,
                "downloaded artifact SHA-256 does not match expectation",
            )
            .with_context("algorithm", "sha256"));
        }
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphene_core::{ArtifactIntegrity, ArtifactSource, Sha1Digest, Sha256Digest};
    use std::path::PathBuf;

    fn transfer() -> TransferResult {
        TransferResult {
            path: PathBuf::from("fixture.part"),
            bytes: 3,
            sha1: "a9993e364706816aba3e25717850c26c9cd0d89d"
                .parse::<Sha1Digest>()
                .expect("sha1"),
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
                .parse::<Sha256Digest>()
                .expect("sha256"),
            source_host: Some("localhost".into()),
        }
    }

    #[test]
    fn verifies_size_sha1_and_sha256_together() {
        let artifact = Artifact::new(
            vec![ArtifactSource::new("http://localhost")],
            ArtifactIntegrity::none()
                .with_sha1(
                    "a9993e364706816aba3e25717850c26c9cd0d89d"
                        .parse()
                        .expect("sha1"),
                )
                .with_sha256(
                    "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
                        .parse()
                        .expect("sha256"),
                ),
        )
        .with_expected_size(3);
        verify_transfer(&artifact, &transfer()).expect("verification succeeds");
    }

    #[test]
    fn verification_reports_stable_mismatch_codes() {
        let size = Artifact::new(
            Vec::new(),
            ArtifactIntegrity::none().with_sha256(
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
                    .parse()
                    .expect("sha256"),
            ),
        )
        .with_expected_size(4);
        assert_eq!(
            verify_transfer(&size, &transfer())
                .expect_err("size mismatch")
                .code,
            ErrorCode::DownloadSizeMismatch
        );

        let hash = Artifact::new(
            Vec::new(),
            ArtifactIntegrity::none().with_sha1(
                "0000000000000000000000000000000000000000"
                    .parse()
                    .expect("sha1"),
            ),
        );
        assert_eq!(
            verify_transfer(&hash, &transfer())
                .expect_err("hash mismatch")
                .code,
            ErrorCode::HashMismatch
        );
    }
}
