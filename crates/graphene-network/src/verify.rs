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

    match artifact.integrity.sha512() {
        Some(expected) if transfer.sha512 != expected => {
            return Err(GrapheneError::new(
                ErrorCode::HashMismatch,
                ErrorKind::Integrity,
                "downloaded artifact SHA-512 does not match expectation",
            )
            .with_context("algorithm", "sha512"));
        }
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphene_core::{
        ArtifactIntegrity, ArtifactSource, Sha1Digest, Sha256Digest, Sha512Digest,
    };
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
            sha512: "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
                     2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
                .parse::<Sha512Digest>()
                .expect("sha512"),
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

    #[test]
    fn sha512_mismatch_fails_even_when_other_digests_match() {
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
                )
                .with_sha512(
                    "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce\
                     47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
                        .parse()
                        .expect("sha512"),
                ),
        )
        .with_expected_size(3);

        let error = verify_transfer(&artifact, &transfer()).expect_err("sha512 mismatch");
        assert_eq!(error.code, ErrorCode::HashMismatch);
        assert_eq!(error.context.get("algorithm"), Some("sha512"));
    }

    #[test]
    fn declared_sha512_only_artifact_verifies_from_observed_digest() {
        let artifact = Artifact::new(
            Vec::new(),
            ArtifactIntegrity::none().with_sha512(
                "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
                 2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
                    .parse()
                    .expect("sha512"),
            ),
        )
        .with_expected_size(3);
        verify_transfer(&artifact, &transfer()).expect("sha512-only verification succeeds");
    }
}
