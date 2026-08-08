use super::spawn_blocking_install;
use crate::error::install_error;
use graphene_core::{ErrorCode, Result};
use graphene_instance::{InstallReceipt, InstanceDescriptor};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub(super) async fn write_instance_metadata(
    staging: &Path,
    descriptor: &InstanceDescriptor,
    receipt: &InstallReceipt,
) -> Result<()> {
    descriptor.validate().map_err(|source| {
        install_error(
            ErrorCode::InstallValidationFailed,
            "instance descriptor is invalid",
        )
        .with_source(source)
    })?;

    let instance_json = serde_json::to_vec_pretty(descriptor).map_err(|source| {
        install_error(
            ErrorCode::InstallValidationFailed,
            "failed to serialize instance descriptor",
        )
        .with_source(source)
    })?;
    let receipt_json = receipt.to_pretty_json()?;
    let graphene = staging.join(".graphene");

    tokio::fs::create_dir_all(&graphene)
        .await
        .map_err(|source| {
            install_error(
                ErrorCode::InstallStageFailed,
                "failed to create staged Graphene metadata directory",
            )
            .with_source(source)
        })?;

    write_new_sync(staging.join("instance.json"), instance_json).await?;
    write_new_sync(graphene.join("install.json"), receipt_json).await
}

async fn write_new_sync(path: PathBuf, bytes: Vec<u8>) -> Result<()> {
    spawn_blocking_install(move || {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|source| {
                install_error(
                    ErrorCode::InstallStageFailed,
                    "failed to create staged metadata file",
                )
                .with_source(source)
            })?;

        file.write_all(&bytes).map_err(|source| {
            install_error(
                ErrorCode::InstallStageFailed,
                "failed to write staged metadata file",
            )
            .with_source(source)
        })?;

        file.sync_all().map_err(|source| {
            install_error(
                ErrorCode::InstallStageFailed,
                "failed to sync staged metadata file",
            )
            .with_source(source)
        })?;

        Ok(())
    })
    .await
}
