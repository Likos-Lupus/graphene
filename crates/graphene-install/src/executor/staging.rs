use super::spawn_blocking_install;
use crate::{error::install_error, plan::InstallPlan};
use graphene_core::{ErrorCode, Result};
use graphene_instance::{
    InstallReceipt, InstanceDescriptor, InstanceLockfile, ManagedRelativePath as ReceiptPath,
};
use std::{
    fs,
    io::ErrorKind as IoErrorKind,
    path::{Path, PathBuf},
};

pub(super) async fn validate_staging(
    data_root: &Path,
    staging: &Path,
    plan: &InstallPlan,
) -> Result<()> {
    let descriptor_bytes = tokio::fs::read(staging.join("instance.json"))
        .await
        .map_err(|source| {
            install_error(
                ErrorCode::InstallValidationFailed,
                "staged instance descriptor is unreadable",
            )
            .with_source(source)
        })?;

    let descriptor: InstanceDescriptor =
        serde_json::from_slice(&descriptor_bytes).map_err(|source| {
            install_error(
                ErrorCode::InstallValidationFailed,
                "staged instance descriptor does not round-trip",
            )
            .with_source(source)
        })?;

    descriptor.validate().map_err(|source| {
        install_error(
            ErrorCode::InstallValidationFailed,
            "staged instance descriptor is invalid",
        )
        .with_source(source)
    })?;

    if descriptor != plan.instance.descriptor {
        return Err(install_error(
            ErrorCode::InstallValidationFailed,
            "staged instance descriptor differs from the plan",
        ));
    }

    let receipt_bytes = tokio::fs::read(staging.join(".graphene/install.json"))
        .await
        .map_err(|source| {
            install_error(
                ErrorCode::InstallValidationFailed,
                "staged install receipt is unreadable",
            )
            .with_source(source)
        })?;
    let receipt = InstallReceipt::from_json(&receipt_bytes).map_err(|source| {
        install_error(
            ErrorCode::InstallValidationFailed,
            "staged install receipt does not round-trip",
        )
        .with_source(source)
    })?;

    if receipt != plan.receipt {
        return Err(install_error(
            ErrorCode::InstallValidationFailed,
            "staged install receipt differs from the plan",
        ));
    }

    let lockfile_bytes = tokio::fs::read(staging.join(".graphene/lock.json"))
        .await
        .map_err(|source| {
            install_error(
                ErrorCode::InstallValidationFailed,
                "staged instance lockfile is unreadable",
            )
            .with_source(source)
        })?;
    let lockfile: InstanceLockfile = serde_json::from_slice(&lockfile_bytes).map_err(|source| {
        install_error(
            ErrorCode::InstallValidationFailed,
            "staged instance lockfile does not round-trip",
        )
        .with_source(source)
    })?;
    lockfile.validate().map_err(|source| {
        install_error(
            ErrorCode::InstallValidationFailed,
            "staged instance lockfile is invalid",
        )
        .with_source(source)
    })?;

    if lockfile.instance_id != plan.instance.descriptor.instance_id {
        return Err(install_error(
            ErrorCode::InstallValidationFailed,
            "staged instance lockfile identity does not match plan",
        ));
    }

    let mut required_files = Vec::with_capacity(receipt.libraries.len() + 3);
    required_files.push(receipt.client.path.under_instance(staging));
    for library in &receipt.libraries {
        if let Some(classpath) = &library.classpath {
            required_files.push(resolve_receipt_path(data_root, staging, &classpath.path)?);
        }
    }
    required_files.push(resolve_receipt_path(
        data_root,
        staging,
        &receipt.asset_index.path,
    )?);
    if let Some(logging) = &receipt.logging_configuration {
        required_files.push(resolve_receipt_path(data_root, staging, &logging.path)?);
    }

    let native_path = (!plan.native_extractions.is_empty())
        .then(|| staging.join(receipt.natives_directory.as_str()));
    spawn_blocking_install(move || {
        for path in &required_files {
            validate_ordinary_file(path)?;
        }
        if let Some(native_path) = native_path {
            let metadata = fs::symlink_metadata(&native_path).map_err(|source| {
                install_error(
                    ErrorCode::InstallValidationFailed,
                    "staged native directory is missing",
                )
                .with_source(source)
            })?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(install_error(
                    ErrorCode::InstallValidationFailed,
                    "staged native directory is unsafe",
                ));
            }
        }
        Ok(())
    })
    .await
}

trait InstancePathExt {
    fn under_instance(&self, root: &Path) -> PathBuf;
}
impl InstancePathExt for ReceiptPath {
    fn under_instance(&self, root: &Path) -> PathBuf {
        root.join(self.as_str())
    }
}

fn resolve_receipt_path(
    data_root: &Path,
    instance_root: &Path,
    path: &ReceiptPath,
) -> Result<PathBuf> {
    if path.as_str().starts_with("shared/") {
        Ok(data_root.join(path.as_str()))
    } else if path.as_str().starts_with(".minecraft/") || path.as_str().starts_with(".graphene/") {
        Ok(instance_root.join(path.as_str()))
    } else {
        Err(install_error(
            ErrorCode::InstallValidationFailed,
            "receipt path is outside recognized managed roots",
        ))
    }
}

fn validate_ordinary_file(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|source| {
        install_error(
            ErrorCode::InstallValidationFailed,
            "staged required file is missing",
        )
        .with_source(source)
    })?;

    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(install_error(
            ErrorCode::InstallValidationFailed,
            "staged required path is not an ordinary file",
        ));
    }

    Ok(())
}

pub(super) async fn remove_tree_blocking(path: PathBuf) -> Result<()> {
    spawn_blocking_install(move || match fs::remove_dir_all(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == IoErrorKind::NotFound => Ok(()),
        Err(source) => Err(install_error(
            ErrorCode::InstallStageFailed,
            "failed to clean instance staging directory",
        )
        .with_source(source)),
    })
    .await
}
