use crate::{
    acquisition::AcquiredArtifact,
    error::{cancelled_error, install_error},
};
use graphene_core::{
    Artifact, ArtifactId, CancellationToken, ErrorCode, Result, Sha1Digest, Sha256Digest,
};
use graphene_platform::{ManagedRelativePath, ensure_managed_directory};
use sha1::{Digest as _, Sha1};
use sha2::Sha256;
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::Path,
};

pub(super) fn validate_acquired(expected: &Artifact, actual: &AcquiredArtifact) -> Result<()> {
    if expected.id != actual.artifact_id {
        return Err(install_error(
            ErrorCode::InstallValidationFailed,
            "artifact acquirer returned the wrong artifact identity",
        ));
    }

    if expected
        .expected_size
        .is_some_and(|size| size != actual.bytes)
        || expected
            .integrity
            .sha1()
            .is_some_and(|digest| digest != actual.sha1)
        || expected
            .integrity
            .sha256()
            .is_some_and(|digest| digest != actual.sha256)
    {
        return Err(install_error(
            ErrorCode::InstallValidationFailed,
            "artifact acquirer result does not match declared integrity",
        )
        .with_context("artifact_id", expected.id.to_string()));
    }

    let metadata = fs::symlink_metadata(&actual.path).map_err(|source| {
        install_error(
            ErrorCode::InstallValidationFailed,
            "acquired artifact path is unavailable",
        )
        .with_source(source)
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(install_error(
            ErrorCode::InstallValidationFailed,
            "acquired artifact is not an ordinary file",
        ));
    }

    Ok(())
}

pub(super) fn acquired_entry(
    acquired: &HashMap<ArtifactId, AcquiredArtifact>,
    id: ArtifactId,
) -> Result<&AcquiredArtifact> {
    acquired.get(&id).ok_or_else(|| {
        install_error(
            ErrorCode::InstallStageFailed,
            "planned artifact was not acquired",
        )
        .with_context("artifact_id", id.to_string())
    })
}

pub(super) fn acquired_source(
    acquired: &HashMap<ArtifactId, AcquiredArtifact>,
    id: ArtifactId,
) -> Result<&Path> {
    Ok(acquired_entry(acquired, id)?.path.as_path())
}

#[derive(Debug, Clone, Copy)]
pub(super) struct MaterializedIntegrity {
    pub(super) bytes: u64,
    pub(super) sha1: Sha1Digest,
    pub(super) sha256: Sha256Digest,
}

pub(super) fn materialize_file(
    source: &Path,
    root: &Path,
    relative: &ManagedRelativePath,
    shared_integrity: Option<MaterializedIntegrity>,
    cancellation: &CancellationToken,
) -> Result<()> {
    let parent_relative = relative.as_path().parent().ok_or_else(|| {
        install_error(
            ErrorCode::InstallStageFailed,
            "materialization target has no parent",
        )
    })?;
    let parent = if parent_relative.as_os_str().is_empty() {
        root.to_path_buf()
    } else {
        ensure_managed_directory(root, &ManagedRelativePath::new(parent_relative)?)?
    };

    let file_name = relative.as_path().file_name().ok_or_else(|| {
        install_error(
            ErrorCode::InstallStageFailed,
            "materialization target has no file name",
        )
    })?;
    let destination = parent.join(file_name);

    if let Ok(metadata) = fs::symlink_metadata(&destination) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(install_error(
                ErrorCode::InstallStageFailed,
                "materialization target is not an ordinary file",
            ));
        }

        let Some(expected) = shared_integrity else {
            return Err(install_error(
                ErrorCode::InstallStageFailed,
                "instance staging materialization target already exists",
            ));
        };

        if verify_materialized_file(&destination, expected, cancellation)? {
            return Ok(());
        }
    }

    let temporary = parent.join(format!(".graphene-materialize-{}.part", ArtifactId::new()));
    let result = (|| -> Result<()> {
        let mut input = fs::File::open(source).map_err(|source| {
            install_error(
                ErrorCode::InstallStageFailed,
                "failed to open verified artifact for materialization",
            )
            .with_source(source)
        })?;

        let mut output = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|source| {
                install_error(
                    ErrorCode::InstallStageFailed,
                    "failed to create materialization temporary file",
                )
                .with_source(source)
            })?;

        let mut buffer = [0_u8; 256 * 1024];
        loop {
            if cancellation.is_cancelled() {
                return Err(cancelled_error());
            }

            let read = input.read(&mut buffer).map_err(|source| {
                install_error(
                    ErrorCode::InstallStageFailed,
                    "failed to read verified artifact for materialization",
                )
                .with_source(source)
            })?;

            if read == 0 {
                break;
            }

            output.write_all(&buffer[..read]).map_err(|source| {
                install_error(
                    ErrorCode::InstallStageFailed,
                    "failed to copy verified artifact into materialization",
                )
                .with_source(source)
            })?;
        }

        output.sync_all().map_err(|source| {
            install_error(
                ErrorCode::InstallStageFailed,
                "failed to sync materialized artifact",
            )
            .with_source(source)
        })?;

        drop(output);
        if let Some(expected) = shared_integrity {
            if !verify_materialized_file(&temporary, expected, cancellation)? {
                return Err(install_error(
                    ErrorCode::InstallValidationFailed,
                    "materialized shared artifact failed integrity verification",
                ));
            }

            graphene_platform::replace_file_safely(&temporary, &destination).map_err(|source| {
                install_error(
                    ErrorCode::InstallStageFailed,
                    "failed to publish shared materialization",
                )
                .with_source(source)
            })?;
        } else {
            fs::rename(&temporary, &destination).map_err(|source| {
                install_error(
                    ErrorCode::InstallStageFailed,
                    "failed to publish staged materialization",
                )
                .with_source(source)
            })?;
        }

        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }

    result
}

fn verify_materialized_file(
    path: &Path,
    expected: MaterializedIntegrity,
    cancellation: &CancellationToken,
) -> Result<bool> {
    let metadata = fs::symlink_metadata(path).map_err(|source| {
        install_error(
            ErrorCode::InstallValidationFailed,
            "failed to inspect materialized artifact",
        )
        .with_source(source)
    })?;

    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() != expected.bytes
    {
        return Ok(false);
    }

    let mut file = fs::File::open(path).map_err(|source| {
        install_error(
            ErrorCode::InstallValidationFailed,
            "failed to open materialized artifact for verification",
        )
        .with_source(source)
    })?;
    let mut sha1 = Sha1::new();
    let mut sha256 = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancellation.is_cancelled() {
            return Err(cancelled_error());
        }

        let read = file.read(&mut buffer).map_err(|source| {
            install_error(
                ErrorCode::InstallValidationFailed,
                "failed to hash materialized artifact",
            )
            .with_source(source)
        })?;

        if read == 0 {
            break;
        }

        sha1.update(&buffer[..read]);
        sha256.update(&buffer[..read]);
    }

    let actual_sha1: [u8; 20] = sha1.finalize().into();
    let actual_sha256: [u8; 32] = sha256.finalize().into();

    Ok(actual_sha1 == *expected.sha1.as_bytes() && actual_sha256 == *expected.sha256.as_bytes())
}
