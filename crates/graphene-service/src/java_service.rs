use crate::{ArtifactService, context::ServiceContext, operation_lifecycle};
use graphene_core::{
    ErrorCode, ErrorKind, GrapheneError, InstanceId, OperationController, OperationHandle, Result,
};
use graphene_install::{extract_managed_tar_gz, extract_managed_zip};
use graphene_instance::InstallReceipt;
use graphene_java::{
    JavaArchitecture, JavaCandidate, JavaCandidateSource, JavaRequirement, JavaRuntime,
    MANAGED_RUNTIME_SCHEMA_VERSION, ManagedArchiveFormat, ManagedJavaInstallPlan,
    ManagedJavaRequest, ManagedJavaRuntime, is_compatible, probe_java, select_java,
    select_managed_runtime,
};
use graphene_storage::ManagedRuntimeStore;
use std::{
    fs,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};

#[derive(Clone)]
pub struct JavaService {
    context: Arc<ServiceContext>,
}

impl std::fmt::Debug for JavaService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JavaService").finish_non_exhaustive()
    }
}

impl JavaService {
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        Self { context }
    }

    /// Side-effect-free selection: explicit override, compatible local Java, then committed managed Java.
    pub async fn select_for_instance(
        &self,
        instance_id: InstanceId,
        explicit: Option<PathBuf>,
    ) -> Result<JavaRuntime> {
        let requirement = self.requirement_for_instance(instance_id).await?;
        if explicit.is_some() {
            return select_java(&requirement, explicit).await;
        }

        match select_java(&requirement, None).await {
            Ok(runtime) => Ok(runtime),
            Err(local_error)
                if matches!(
                    local_error.code,
                    ErrorCode::JavaNotFound | ErrorCode::JavaIncompatible
                ) =>
            {
                match self.select_committed_managed(&requirement).await {
                    Ok(runtime) => Ok(runtime),
                    Err(managed_error) if managed_error.code == ErrorCode::JavaNotFound => {
                        Err(local_error)
                    }
                    Err(managed_error) => Err(managed_error),
                }
            }
            Err(error) => Err(error),
        }
    }

    /// Explicitly permits managed installation when no existing runtime satisfies the instance.
    #[must_use]
    pub fn ensure_for_instance(
        &self,
        instance_id: InstanceId,
        explicit: Option<PathBuf>,
    ) -> ManagedJavaOperation {
        let controller = self.context.operations.create("ensure-java");
        let operation = controller.handle();
        let service = self.clone();
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = async {
                controller.set_stage("inspect_local_runtimes")?;
                let requirement = service.requirement_for_instance(instance_id).await?;
                if explicit.is_some() {
                    return select_java(&requirement, explicit).await;
                }

                match select_java(&requirement, None).await {
                    Ok(runtime) => return Ok(runtime),
                    Err(error)
                        if matches!(
                            error.code,
                            ErrorCode::JavaNotFound | ErrorCode::JavaIncompatible
                        ) => {}
                    Err(error) => return Err(error),
                }

                controller.set_stage("inspect_managed_runtimes")?;
                match service.select_committed_managed(&requirement).await {
                    Ok(runtime) => return Ok(runtime),
                    Err(error) if error.code == ErrorCode::JavaNotFound => {}
                    Err(error) => return Err(error),
                }

                service
                    .install_managed_inner(requirement, &controller)
                    .await
            }
            .await;

            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::OperationCancelled,
                ErrorKind::Java,
                "managed Java ensure operation was cancelled",
            )
        });

        ManagedJavaOperation { operation, future }
    }

    /// Explicit managed-runtime installation operation for an already normalized requirement.
    #[must_use]
    pub fn install_managed(&self, requirement: JavaRequirement) -> ManagedJavaOperation {
        let controller = self.context.operations.create("install-managed-java");
        let operation = controller.handle();
        let service = self.clone();
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = service
                .install_managed_inner(requirement, &controller)
                .await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::OperationCancelled,
                ErrorKind::Java,
                "managed Java installation was cancelled",
            )
        });
        ManagedJavaOperation { operation, future }
    }

    pub async fn managed_runtimes(&self) -> Result<Vec<ManagedJavaRuntime>> {
        let root = self.context.storage.clone();
        spawn_blocking_java(move || load_inventory(&root)).await
    }

    async fn requirement_for_instance(&self, instance_id: InstanceId) -> Result<JavaRequirement> {
        let receipt_path = self
            .context
            .storage
            .path()
            .join("instances")
            .join(instance_id.to_string())
            .join(".graphene/install.json");
        let bytes = tokio::fs::read(&receipt_path).await.map_err(|source| {
            GrapheneError::new(
                ErrorCode::LaunchInstanceInvalid,
                ErrorKind::Launch,
                "committed install receipt is unavailable for Java selection",
            )
            .with_source(source)
        })?;

        let receipt = InstallReceipt::from_json(&bytes)?;
        if receipt.instance_id != instance_id {
            return Err(GrapheneError::new(
                ErrorCode::LaunchInstanceInvalid,
                ErrorKind::Launch,
                "install receipt identity does not match Java selection request",
            ));
        }

        Ok(JavaRequirement {
            major_version: receipt.java_requirement.major_version,
            component_hint: receipt.java_requirement.component_hint.clone(),
        })
    }

    async fn select_committed_managed(&self, requirement: &JavaRequirement) -> Result<JavaRuntime> {
        let runtimes = self.managed_runtimes().await?;
        let descriptor =
            select_managed_runtime(requirement, JavaArchitecture::current(), &runtimes)
                .ok_or_else(|| {
                    java_error(
                        ErrorCode::JavaNotFound,
                        "no compatible committed managed Java runtime was found",
                    )
                })?;
        let runtime_dir = self
            .context
            .storage
            .path()
            .join("shared/runtimes")
            .join(descriptor.id.to_string());
        validate_and_probe_committed(descriptor, runtime_dir, requirement).await
    }

    async fn install_managed_inner(
        &self,
        requirement: JavaRequirement,
        controller: &OperationController,
    ) -> Result<JavaRuntime> {
        if controller.is_cancelled() {
            return Err(cancelled());
        }

        controller.set_stage("resolve_managed_release")?;
        let request = ManagedJavaRequest::for_current_platform(&requirement)?;
        let release = self
            .context
            .java_distribution_provider
            .resolve_release(&request, controller)
            .await?;
        let plan = ManagedJavaInstallPlan::new(request, release)?;
        let runtime_id = plan.release.runtime_id;
        let gate = self.context.runtime_gate(runtime_id);
        let token = controller.cancellation_token();
        let _guard = tokio::select! { () = token.cancelled() => return Err(cancelled()), guard = gate.lock() => guard };

        let runtime_dir = self
            .context
            .storage
            .path()
            .join("shared/runtimes")
            .join(runtime_id.to_string());
        if let Some(existing) = self.load_one_runtime(runtime_id).await? {
            return validate_and_probe_committed(existing, runtime_dir, &requirement).await;
        }

        if runtime_dir_exists(runtime_dir.clone()).await? {
            return Err(java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "managed runtime directory exists without a valid descriptor",
            ));
        }

        controller.set_stage("download_archive")?;
        let acquired = ArtifactService::new(Arc::clone(&self.context))
            .acquire(plan.release.artifact.clone(), Some(&controller.handle()))
            .await_result()
            .await?;
        controller.set_stage("verify_archive")?;
        if acquired.sha256 != plan.release.archive_sha256 {
            return Err(java_error(
                ErrorCode::HashMismatch,
                "managed Java artifact checksum does not match release plan",
            ));
        }

        if controller.is_cancelled() {
            return Err(cancelled());
        }

        controller.set_stage("extract_runtime")?;
        let root = self.context.storage.clone();
        let operation_id = controller.handle().id();
        let archive = acquired.path.clone();
        let format = plan.release.archive_format;
        let cancellation = controller.cancellation_token();
        let staging = spawn_blocking_java(move || {
            let store = ManagedRuntimeStore::new(&root)?;
            let staging = store.prepare_staging(operation_id)?;
            let payload = match store.payload_dir(operation_id) {
                Ok(path) => path,
                Err(error) => {
                    let _ = store.cleanup_staging(operation_id);
                    return Err(error);
                }
            };

            let extracted = match format {
                ManagedArchiveFormat::Zip => extract_managed_zip(&archive, &payload, &cancellation),
                ManagedArchiveFormat::TarGz => {
                    extract_managed_tar_gz(&archive, &payload, &cancellation)
                }
                _ => Err(java_error(
                    ErrorCode::JavaManagedArchiveInvalid,
                    "managed Java install plan uses an unsupported archive format",
                )),
            };

            if let Err(source) = extracted {
                let _ = store.cleanup_staging(operation_id);
                if source.is_cancelled() {
                    return Err(cancelled());
                }
                return Err(java_error(
                    ErrorCode::JavaManagedArchiveInvalid,
                    "managed Java archive extraction failed",
                )
                .with_source(source));
            }
            Ok(staging)
        })
        .await?;

        let result = self
            .finish_staged_runtime(&plan, &requirement, controller, staging)
            .await;
        if result.is_err() {
            let root = self.context.storage.clone();
            let _ = spawn_blocking_java(move || {
                ManagedRuntimeStore::new(&root)?.cleanup_staging(operation_id)
            })
            .await;
        }
        result
    }

    async fn finish_staged_runtime(
        &self,
        plan: &ManagedJavaInstallPlan,
        requirement: &JavaRequirement,
        controller: &OperationController,
        staging: PathBuf,
    ) -> Result<JavaRuntime> {
        if controller.is_cancelled() {
            return Err(cancelled());
        }

        controller.set_stage("probe_runtime")?;
        let search_root = staging.join("runtime");
        let candidate_path =
            spawn_blocking_java(move || locate_java_executable(&search_root)).await?;
        make_executable(&candidate_path).await?;
        let candidate = JavaCandidate {
            executable: candidate_path.clone(),
            source: JavaCandidateSource::Managed,
        };
        let probed = probe_java(&candidate).await.map_err(|source| {
            java_error(
                ErrorCode::JavaRuntimeUnexecutable,
                "staged managed Java executable failed bounded probe",
            )
            .with_source(source)
        })?;

        if !is_compatible(&probed, requirement, JavaArchitecture::current())
            || probed.architecture != plan.release.architecture
        {
            let code = if probed.architecture != plan.release.architecture {
                ErrorCode::JavaArchitectureMismatch
            } else if probed.major_version < requirement.major_version {
                ErrorCode::JavaVersionTooOld
            } else if probed.major_version > requirement.major_version {
                ErrorCode::JavaVersionTooNew
            } else {
                ErrorCode::JavaIncompatible
            };
            return Err(java_error(
                code,
                "staged managed Java runtime does not satisfy the requested compatibility",
            ));
        }

        let relative = candidate_path.strip_prefix(&staging).map_err(|_| {
            java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "staged Java executable escaped runtime staging",
            )
        })?;
        let executable_relative_path = relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let descriptor = ManagedJavaRuntime {
            schema_version: MANAGED_RUNTIME_SCHEMA_VERSION,
            id: plan.release.runtime_id,
            provider: plan.release.provider.clone(),
            distribution: plan.release.distribution.clone(),
            release_name: plan.release.release_name.clone(),
            version: probed.version.clone(),
            major_version: probed.major_version,
            vendor: probed.vendor.clone(),
            os: plan.release.os,
            architecture: probed.architecture,
            image: plan.release.image,
            archive_sha256: plan.release.archive_sha256,
            executable_relative_path,
        };
        descriptor.validate()?;
        controller.set_stage("commit")?;
        let bytes = serde_json::to_vec_pretty(&descriptor).map_err(|source| {
            java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "failed to serialize managed runtime descriptor",
            )
            .with_source(source)
        })?;
        let root = self.context.storage.clone();
        let operation_id = controller.handle().id();
        let descriptor_for_validation = descriptor.clone();

        spawn_blocking_java(move || {
            let store = ManagedRuntimeStore::new(&root)?;
            store.write_staging_descriptor(operation_id, &bytes)?;
            let reread = fs::read(store.staging_dir(operation_id).join("runtime.json")).map_err(
                |source| {
                    java_error(
                        ErrorCode::JavaManagedRuntimeCorrupt,
                        "failed to re-read staged runtime descriptor",
                    )
                    .with_source(source)
                },
            )?;
            let roundtrip: ManagedJavaRuntime =
                serde_json::from_slice(&reread).map_err(|source| {
                    java_error(
                        ErrorCode::JavaManagedRuntimeCorrupt,
                        "staged runtime descriptor does not round-trip",
                    )
                    .with_source(source)
                })?;
            roundtrip.validate()?;
            if roundtrip != descriptor_for_validation {
                return Err(java_error(
                    ErrorCode::JavaManagedRuntimeCorrupt,
                    "staged runtime descriptor changed before publication",
                ));
            }
            Ok(())
        })
        .await?;

        tokio::task::yield_now().await;
        if controller.is_cancelled() {
            return Err(cancelled());
        }

        if !controller.seal_cancellation() {
            return Err(cancelled());
        }

        let root = self.context.storage.clone();
        let runtime_id = descriptor.id;
        spawn_blocking_java(move || {
            let store = ManagedRuntimeStore::new(&root)?;
            match store.publish(operation_id, runtime_id) {
                Ok(()) => Ok(()),
                Err(error) if store.runtime_dir(runtime_id).is_dir() => {
                    let _ = store.cleanup_staging(operation_id);
                    Err(java_error(
                        ErrorCode::JavaManagedInstallFailed,
                        "managed runtime identity was published concurrently; retry selection",
                    )
                    .with_source(error))
                }
                Err(error) => Err(error),
            }
        })
        .await?;

        let committed = self
            .context
            .storage
            .path()
            .join("shared/runtimes")
            .join(descriptor.id.to_string());
        descriptor.to_java_runtime(&committed)
    }

    async fn load_one_runtime(
        &self,
        id: graphene_core::ManagedRuntimeId,
    ) -> Result<Option<ManagedJavaRuntime>> {
        let root = self.context.storage.clone();
        spawn_blocking_java(move || {
            let store = ManagedRuntimeStore::new(&root)?;
            let Some(bytes) = store.read_descriptor(id)? else {
                return Ok(None);
            };

            let descriptor: ManagedJavaRuntime =
                serde_json::from_slice(&bytes).map_err(|source| {
                    java_error(
                        ErrorCode::JavaManagedRuntimeCorrupt,
                        "managed runtime descriptor JSON is malformed",
                    )
                    .with_source(source)
                })?;

            descriptor.validate()?;
            Ok(Some(descriptor))
        })
        .await
    }
}

pub struct ManagedJavaOperation {
    operation: OperationHandle,
    future: Pin<Box<dyn Future<Output = Result<JavaRuntime>> + Send>>,
}

impl ManagedJavaOperation {
    #[must_use]
    pub fn operation(&self) -> OperationHandle {
        self.operation.clone()
    }
    pub async fn await_result(self) -> Result<JavaRuntime> {
        self.future.await
    }
}

impl std::fmt::Debug for ManagedJavaOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedJavaOperation")
            .field("operation_id", &self.operation.id())
            .finish_non_exhaustive()
    }
}

fn load_inventory(root: &graphene_storage::DataRoot) -> Result<Vec<ManagedJavaRuntime>> {
    let store = ManagedRuntimeStore::new(root)?;
    let mut runtimes = Vec::new();

    for id in store.list_ids()? {
        let Some(bytes) = store.read_descriptor(id)? else {
            continue;
        };

        let descriptor: ManagedJavaRuntime = serde_json::from_slice(&bytes).map_err(|source| {
            java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "managed runtime descriptor JSON is malformed",
            )
            .with_source(source)
        })?;
        descriptor.validate()?;
        if descriptor.id != id {
            return Err(java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "managed runtime descriptor identity mismatches its directory",
            ));
        }

        validate_managed_executable_path(
            &store.runtime_dir(id),
            &descriptor.executable_relative_path,
        )?;
        runtimes.push(descriptor);
    }

    runtimes.sort_by_key(|runtime| runtime.id.to_string());
    Ok(runtimes)
}

async fn runtime_dir_exists(path: PathBuf) -> Result<bool> {
    spawn_blocking_java(move || match fs::symlink_metadata(&path) {
        Ok(metadata) => Ok(metadata.is_dir() && !metadata.file_type().is_symlink()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(java_error(
            ErrorCode::JavaManagedRuntimeCorrupt,
            "failed to inspect managed runtime directory",
        )
        .with_source(source)),
    })
    .await
}

async fn validate_and_probe_committed(
    descriptor: ManagedJavaRuntime,
    runtime_dir: PathBuf,
    requirement: &JavaRequirement,
) -> Result<JavaRuntime> {
    descriptor.validate()?;
    let relative = descriptor.executable_relative_path.clone();
    let validation_root = runtime_dir.clone();
    spawn_blocking_java(move || validate_managed_executable_path(&validation_root, &relative))
        .await?;
    let expected = descriptor.to_java_runtime(&runtime_dir)?;
    let candidate = JavaCandidate {
        executable: expected.executable.clone(),
        source: JavaCandidateSource::Managed,
    };
    let probed = probe_java(&candidate).await.map_err(|source| {
        java_error(
            ErrorCode::JavaManagedRuntimeCorrupt,
            "committed managed Java runtime no longer probes successfully",
        )
        .with_source(source)
    })?;

    if probed.major_version != descriptor.major_version
        || probed.architecture != descriptor.architecture
        || probed.version != descriptor.version
        || probed.vendor != descriptor.vendor
        || !is_compatible(&probed, requirement, JavaArchitecture::current())
    {
        return Err(java_error(
            ErrorCode::JavaManagedRuntimeCorrupt,
            "committed managed Java runtime probe differs from its descriptor",
        ));
    }

    Ok(probed)
}

fn validate_managed_executable_path(runtime_dir: &Path, relative: &str) -> Result<PathBuf> {
    let root_metadata = fs::symlink_metadata(runtime_dir).map_err(|source| {
        java_error(
            ErrorCode::JavaManagedRuntimeCorrupt,
            "managed runtime root is unavailable",
        )
        .with_source(source)
    })?;

    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(java_error(
            ErrorCode::JavaManagedRuntimeCorrupt,
            "managed runtime root is not an ordinary directory",
        ));
    }

    let mut current = runtime_dir.to_path_buf();
    let components: Vec<_> = Path::new(relative).components().collect();
    if components.is_empty() {
        return Err(java_error(
            ErrorCode::JavaManagedRuntimeCorrupt,
            "managed runtime executable path is empty",
        ));
    }

    for (index, component) in components.iter().enumerate() {
        use std::path::Component;
        let Component::Normal(part) = component else {
            return Err(java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "managed runtime executable path is unsafe",
            ));
        };

        current.push(part);
        let metadata = fs::symlink_metadata(&current).map_err(|source| {
            java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "managed runtime executable path is unavailable",
            )
            .with_source(source)
        })?;
        if metadata.file_type().is_symlink() {
            return Err(java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "managed runtime executable path contains a symbolic link",
            ));
        }

        let is_last = index + 1 == components.len();
        if (is_last && !metadata.is_file()) || (!is_last && !metadata.is_dir()) {
            return Err(java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "managed runtime executable path has an invalid entry type",
            ));
        }
    }

    Ok(current)
}

fn locate_java_executable(root: &Path) -> Result<PathBuf> {
    const MAX_SCAN_ENTRIES: usize = 4096;

    let expected_name = if cfg!(windows) { "java.exe" } else { "java" };
    let mut stack = vec![root.to_path_buf()];
    let mut matches = Vec::new();
    let mut seen = 0usize;

    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(&directory).map_err(|source| {
            java_error(
                ErrorCode::JavaManagedRuntimeCorrupt,
                "failed to inspect staged managed runtime",
            )
            .with_source(source)
        })? {
            let entry = entry.map_err(|source| {
                java_error(
                    ErrorCode::JavaManagedRuntimeCorrupt,
                    "failed to inspect staged managed runtime entry",
                )
                .with_source(source)
            })?;
            seen += 1;
            if seen > MAX_SCAN_ENTRIES {
                return Err(java_error(
                    ErrorCode::JavaManagedRuntimeCorrupt,
                    "staged managed runtime contains too many filesystem entries",
                ));
            }

            let ty = entry.file_type().map_err(|source| {
                java_error(
                    ErrorCode::JavaManagedRuntimeCorrupt,
                    "failed to inspect staged runtime entry type",
                )
                .with_source(source)
            })?;

            if ty.is_symlink() {
                return Err(java_error(
                    ErrorCode::JavaManagedRuntimeCorrupt,
                    "staged managed runtime contains a symbolic link",
                ));
            }

            if ty.is_dir() {
                stack.push(entry.path());
            } else if ty.is_file()
                && entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(expected_name)
            {
                let path = entry.path();
                if path
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == "bin")
                {
                    matches.push(path);
                }
            }
        }
    }

    matches.sort();
    matches.into_iter().next().ok_or_else(|| {
        java_error(
            ErrorCode::JavaRuntimeUnexecutable,
            "managed runtime archive does not contain an expected bin/java executable",
        )
    })
}

async fn make_executable(path: &Path) -> Result<()> {
    let path = path.to_path_buf();
    spawn_blocking_java(move || {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::symlink_metadata(&path).map_err(|source| {
                java_error(
                    ErrorCode::JavaRuntimeUnexecutable,
                    "failed to inspect staged Java executable permissions",
                )
                .with_source(source)
            })?;

            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(java_error(
                    ErrorCode::JavaRuntimeUnexecutable,
                    "staged Java executable is not an ordinary file",
                ));
            }

            let mut permissions = metadata.permissions();
            permissions.set_mode(permissions.mode() | 0o500);
            fs::set_permissions(&path, permissions).map_err(|source| {
                java_error(
                    ErrorCode::JavaRuntimeUnexecutable,
                    "failed to set staged Java executable permission",
                )
                .with_source(source)
            })?;
        }

        #[cfg(not(unix))]
        {
            let _ = path;
        }
        Ok(())
    })
    .await
}

async fn spawn_blocking_java<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work).await.map_err(|source| {
        java_error(
            ErrorCode::JavaManagedInstallFailed,
            "blocking managed Java task failed",
        )
        .with_source(source)
    })?
}

fn java_error(code: ErrorCode, message: &'static str) -> GrapheneError {
    GrapheneError::new(code, ErrorKind::Java, message)
}

fn cancelled() -> GrapheneError {
    GrapheneError::new(
        ErrorCode::OperationCancelled,
        ErrorKind::Cancelled,
        "managed Java operation was cancelled",
    )
}
