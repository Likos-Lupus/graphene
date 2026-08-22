use crate::{
    context::ServiceContext, instance_service::repository::InstanceRepository, operation_lifecycle,
};
use graphene_core::{
    ErrorCode, ErrorKind, GrapheneError, InstanceId, OperationController, OperationHandle,
    Progress, Result, Sha1Digest, Sha256Digest,
};
use graphene_instance::{
    FindingCode, FindingSeverity, InstanceLockfile, InstanceStateFingerprint,
    LockedMaterializationScope, ManagedRelativePath, Repairability, VerificationFinding,
    VerificationMode, VerificationReport,
};
use graphene_storage::{MAX_LOCKFILE_BYTES, read_document_bounded};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    future::Future,
    io::Read,
    path::Path,
    pin::Pin,
    sync::Arc,
};

/// Prepared asynchronous instance verification operation handle.
pub struct InstanceVerifyOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = Result<VerificationReport>> + Send>>,
}

impl InstanceVerifyOperation {
    /// Returns the public operation handle for progress observation and cancellation.
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }

    /// Awaits completion of the verification scan.
    pub async fn await_result(self) -> Result<VerificationReport> {
        self.future.await
    }
}

impl std::fmt::Debug for InstanceVerifyOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceVerifyOperation")
            .field("operation_id", &self.operation.id())
            .finish_non_exhaustive()
    }
}

pub(crate) fn start_verify_operation(
    context: Arc<ServiceContext>,
    instance_id: InstanceId,
    mode: VerificationMode,
) -> InstanceVerifyOperation {
    let controller = context.operations.create("instance-verify");
    let operation = controller.handle();
    let future = Box::pin(async move {
        operation_lifecycle::start(&controller)?;
        let result = execute_verify(context, instance_id, mode, &controller).await;
        operation_lifecycle::finish(
            &controller,
            result,
            ErrorCode::OperationCancelled,
            ErrorKind::Cancelled,
            "instance verification operation was cancelled",
        )
    });

    InstanceVerifyOperation { operation, future }
}

pub(crate) async fn execute_verify(
    context: Arc<ServiceContext>,
    instance_id: InstanceId,
    mode: VerificationMode,
    controller: &OperationController,
) -> Result<VerificationReport> {
    controller.set_stage("acquire-lease")?;
    let repo = InstanceRepository::new(context.storage.path());
    let _lease = repo.acquire_shared_lease(instance_id)?;

    controller.set_stage("verify-metadata")?;
    let mut findings = Vec::new();

    // 1. Verify descriptor
    let descriptor = match repo.load_descriptor(instance_id) {
        Ok(desc) => Some(desc),
        Err(err) => {
            findings.push(VerificationFinding::new(
                FindingCode::DescriptorInvalid,
                FindingSeverity::Error,
                format!("instance descriptor is invalid: {}", err.message()),
                false,
            ));
            None
        }
    };

    // 2. Verify receipt
    let receipt = match repo.load_receipt(instance_id) {
        Ok(rec) => Some(rec),
        Err(err) => {
            findings.push(VerificationFinding::new(
                FindingCode::ReceiptInvalid,
                FindingSeverity::Error,
                format!("install receipt is invalid: {}", err.message()),
                false,
            ));
            None
        }
    };

    // 3. Verify lockfile
    let lockfile_path = repo.paths().lockfile_path(instance_id);
    let lockfile = if lockfile_path.exists() {
        match read_document_bounded(&lockfile_path, MAX_LOCKFILE_BYTES) {
            Ok(bytes) => match serde_json::from_slice::<InstanceLockfile>(&bytes) {
                Ok(lock) => match lock.validate() {
                    Ok(()) => Some(lock),
                    Err(err) => {
                        findings.push(VerificationFinding::new(
                            FindingCode::LockfileInvalid,
                            FindingSeverity::Error,
                            format!("instance lockfile validation failed: {}", err.message()),
                            false,
                        ));
                        None
                    }
                },
                Err(err) => {
                    findings.push(VerificationFinding::new(
                        FindingCode::LockfileInvalid,
                        FindingSeverity::Error,
                        format!("instance lockfile deserialization failed: {err}"),
                        false,
                    ));
                    None
                }
            },
            Err(err) => {
                findings.push(VerificationFinding::new(
                    FindingCode::LockfileInvalid,
                    FindingSeverity::Error,
                    format!("instance lockfile is unreadable: {}", err.message()),
                    false,
                ));
                None
            }
        }
    } else {
        findings.push(VerificationFinding::new(
            FindingCode::LegacyInstance,
            FindingSeverity::Warning,
            "instance lacks a Phase 4 desired-state lockfile (legacy instance)",
            false,
        ));
        None
    };

    // Check cross-document ID consistency
    if let (Some(desc), Some(rec)) = (&descriptor, &receipt)
        && (desc.instance_id != instance_id || rec.instance_id != instance_id)
    {
        findings.push(VerificationFinding::new(
            FindingCode::IdentityMismatch,
            FindingSeverity::Error,
            "instance documents have mismatched instance IDs",
            false,
        ));
    }

    controller.set_stage("verify-managed-files")?;
    let data_root = context.storage.path();
    let instance_root = repo.paths().instance_root(instance_id);

    // If lockfile is present, verify all lockfile artifacts
    if let Some(lock) = &lockfile {
        let total_items = lock.artifacts.len() + lock.generated_outputs.len();
        for (index, artifact) in lock.artifacts.iter().enumerate() {
            let full_path = match artifact.scope {
                LockedMaterializationScope::SharedImmutable => {
                    data_root.join(artifact.destination.as_str())
                }
                LockedMaterializationScope::InstanceMutable => {
                    instance_root.join(artifact.destination.as_str())
                }
            };
            verify_file_item(
                &full_path,
                artifact.destination.clone(),
                &artifact.logical_key,
                artifact.expected_size,
                artifact.integrity.sha1().as_ref(),
                artifact.integrity.sha256().as_ref(),
                mode,
                &mut findings,
            );
            let _ = controller.set_progress(Progress::Items {
                completed: index as u64 + 1,
                total: Some(total_items as u64),
            });
        }

        for output in &lock.generated_outputs {
            let full_path = data_root.join(output.destination.as_str());
            verify_file_item(
                &full_path,
                output.destination.clone(),
                &output.component_uid,
                Some(output.size),
                None,
                Some(&output.sha256),
                mode,
                &mut findings,
            );
        }
    } else if let Some(rec) = &receipt {
        // Fallback for legacy instances: verify receipt paths
        let client_path = instance_root.join(rec.client.path.as_str());
        verify_file_item(
            &client_path,
            rec.client.path.clone(),
            "client",
            rec.client.expected_size,
            rec.client.integrity.sha1().as_ref(),
            rec.client.integrity.sha256().as_ref(),
            mode,
            &mut findings,
        );

        for lib in &rec.libraries {
            if let Some(cp) = &lib.classpath {
                let lib_path = data_root.join(cp.path.as_str());
                verify_file_item(
                    &lib_path,
                    cp.path.clone(),
                    &lib.coordinate,
                    cp.expected_size,
                    cp.integrity.sha1().as_ref(),
                    cp.integrity.sha256().as_ref(),
                    mode,
                    &mut findings,
                );
            }
        }
    }

    let effective_config = repo.effective_config(instance_id).ok();
    let state_fingerprint = if let Some(rec) = &receipt {
        InstanceStateFingerprint::compute(
            instance_id,
            rec,
            lockfile.as_ref(),
            effective_config.as_ref(),
        )
    } else {
        let empty_receipt = fallback_empty_receipt(instance_id);
        InstanceStateFingerprint::compute(instance_id, &empty_receipt, None, None)
    };

    let repairability = classify_repairability(&findings, lockfile.is_some());
    let summary = match repairability {
        Repairability::Healthy => "Instance is completely healthy".to_string(),
        Repairability::LocallyRepairable => "Instance is locally repairable".to_string(),
        Repairability::RepairableWithNetwork => {
            "Instance requires downloading missing artifacts".to_string()
        }
        Repairability::RepairableWithPreparation => {
            "Instance requires loader preparation".to_string()
        }
        Repairability::LegacyMigrationRequired => {
            "Instance requires legacy lockfile migration".to_string()
        }
        Repairability::Unrepairable => "Instance corruption is unrepairable".to_string(),
    };

    Ok(VerificationReport {
        instance_id,
        mode,
        state_fingerprint,
        repairability,
        findings,
        summary,
    })
}

fn fallback_empty_receipt(id: InstanceId) -> graphene_instance::InstallReceipt {
    graphene_instance::InstallReceipt {
        schema_version: 0,
        install_format_version: 0,
        instance_id: id,
        requested_version: String::new(),
        resolved_version: String::new(),
        components: Vec::new(),
        version_type: String::new(),
        main_class: String::new(),
        java_requirement: graphene_instance::InstalledJavaRequirement {
            major_version: 0,
            component_hint: None,
        },
        client: graphene_instance::InstalledArtifact {
            path: ManagedRelativePath::new(".minecraft/versions/none/none.jar")
                .expect("valid path"),
            integrity: graphene_core::ArtifactIntegrity::default(),
            expected_size: None,
        },
        libraries: Vec::new(),
        asset_index_id: String::new(),
        asset_index: graphene_instance::InstalledArtifact {
            path: ManagedRelativePath::new("shared/assets/indexes/none.json").expect("valid path"),
            integrity: graphene_core::ArtifactIntegrity::default(),
            expected_size: None,
        },
        assets_root: ManagedRelativePath::new("shared/assets").expect("valid path"),
        natives_directory: ManagedRelativePath::new(".graphene/natives/none").expect("valid path"),
        logging_configuration: None,
        logging_argument: None,
        jvm_arguments: Vec::new(),
        game_arguments: Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn verify_file_item(
    path: &Path,
    rel_path: ManagedRelativePath,
    logical_key: &str,
    expected_size: Option<u64>,
    expected_sha1: Option<&Sha1Digest>,
    expected_sha256: Option<&Sha256Digest>,
    mode: VerificationMode,
    findings: &mut Vec<VerificationFinding>,
) {
    let metadata = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            findings.push(
                VerificationFinding::new(
                    FindingCode::ManagedFileMissing,
                    FindingSeverity::Error,
                    format!("required managed file is missing at {}", rel_path.as_str()),
                    true,
                )
                .with_path(rel_path)
                .with_logical_item(logical_key),
            );
            return;
        }
        Err(source) => {
            findings.push(
                VerificationFinding::new(
                    FindingCode::ManagedFileMissing,
                    FindingSeverity::Error,
                    format!("failed to inspect managed file: {source}"),
                    true,
                )
                .with_path(rel_path)
                .with_logical_item(logical_key),
            );
            return;
        }
    };

    if metadata.file_type().is_symlink() {
        findings.push(
            VerificationFinding::new(
                FindingCode::UnsafeSymlink,
                FindingSeverity::Error,
                format!(
                    "managed file path is a symbolic link at {}",
                    rel_path.as_str()
                ),
                false,
            )
            .with_path(rel_path)
            .with_logical_item(logical_key),
        );
        return;
    }

    if !metadata.is_file() {
        findings.push(
            VerificationFinding::new(
                FindingCode::ManagedFileWrongType,
                FindingSeverity::Error,
                format!(
                    "managed path is not a regular file at {}",
                    rel_path.as_str()
                ),
                false,
            )
            .with_path(rel_path)
            .with_logical_item(logical_key),
        );
        return;
    }

    if let Some(expected) = expected_size
        && metadata.len() != expected
    {
        findings.push(
            VerificationFinding::new(
                FindingCode::ManagedFileSizeMismatch,
                FindingSeverity::Error,
                format!(
                    "file size mismatch for {}: expected {expected} bytes, observed {} bytes",
                    rel_path.as_str(),
                    metadata.len()
                ),
                true,
            )
            .with_path(rel_path)
            .with_logical_item(logical_key),
        );
        return;
    }

    if mode == VerificationMode::Full && (expected_sha1.is_some() || expected_sha256.is_some()) {
        match stream_compute_hashes(path) {
            Ok((actual_sha1, actual_sha256)) => {
                if let Some(exp_sha1) = expected_sha1
                    && &actual_sha1 != exp_sha1
                {
                    findings.push(
                        VerificationFinding::new(
                            FindingCode::ManagedFileHashMismatch,
                            FindingSeverity::Error,
                            format!(
                                "SHA-1 mismatch for {}: expected {exp_sha1}, observed {actual_sha1}",
                                rel_path.as_str()
                            ),
                            true,
                        )
                        .with_path(rel_path)
                        .with_logical_item(logical_key),
                    );
                    return;
                }
                if let Some(exp_sha256) = expected_sha256
                    && &actual_sha256 != exp_sha256
                {
                    findings.push(
                        VerificationFinding::new(
                            FindingCode::ManagedFileHashMismatch,
                            FindingSeverity::Error,
                            format!(
                                "SHA-256 mismatch for {}: expected {exp_sha256}, observed {actual_sha256}",
                                rel_path.as_str()
                            ),
                            true,
                        )
                        .with_path(rel_path)
                        .with_logical_item(logical_key),
                    );
                }
            }
            Err(err) => {
                findings.push(
                    VerificationFinding::new(
                        FindingCode::ManagedFileHashMismatch,
                        FindingSeverity::Error,
                        format!("failed to compute file hashes: {}", err.message()),
                        true,
                    )
                    .with_path(rel_path)
                    .with_logical_item(logical_key),
                );
            }
        }
    }
}

pub(crate) fn stream_compute_hashes(path: &Path) -> Result<(Sha1Digest, Sha256Digest)> {
    let mut file = File::open(path).map_err(|source| {
        GrapheneError::new(
            ErrorCode::FileOpenFailed,
            ErrorKind::Filesystem,
            "failed to open file for hash verification",
        )
        .with_context("path", path.display().to_string())
        .with_source(source)
    })?;

    let mut sha1 = Sha1::new();
    let mut sha256 = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let read = file.read(&mut buffer).map_err(|source| {
            GrapheneError::new(
                ErrorCode::FileOpenFailed,
                ErrorKind::Filesystem,
                "failed to read file during hash verification",
            )
            .with_context("path", path.display().to_string())
            .with_source(source)
        })?;
        if read == 0 {
            break;
        }
        sha1.update(&buffer[..read]);
        sha256.update(&buffer[..read]);
    }

    let sha1_bytes: [u8; 20] = sha1.finalize().into();
    let sha256_bytes: [u8; 32] = sha256.finalize().into();

    Ok((
        Sha1Digest::from_bytes(sha1_bytes),
        Sha256Digest::from_bytes(sha256_bytes),
    ))
}

fn classify_repairability(findings: &[VerificationFinding], has_lockfile: bool) -> Repairability {
    let error_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.severity == FindingSeverity::Error)
        .collect();

    if error_findings.is_empty() {
        if !has_lockfile {
            return Repairability::LegacyMigrationRequired;
        }
        return Repairability::Healthy;
    }

    if !has_lockfile {
        return Repairability::LegacyMigrationRequired;
    }

    if error_findings.iter().any(|f| !f.repairable) {
        return Repairability::Unrepairable;
    }

    if error_findings.iter().any(|f| {
        matches!(
            f.code,
            FindingCode::GeneratedOutputMissing | FindingCode::GeneratedOutputMismatch
        )
    }) {
        return Repairability::RepairableWithPreparation;
    }

    Repairability::RepairableWithNetwork
}
