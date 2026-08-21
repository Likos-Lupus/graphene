use crate::{context::ServiceContext, java_service::JavaService};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use graphene_install::{
    InstallToolRunner, SelectedToolJava, ToolJavaFuture, ToolRunRequest, ToolRunResult,
    ToolRunnerFuture,
};
use graphene_java::{
    JavaArchitecture, JavaRequirement, JavaRuntime, discover_java_candidates, probe_java,
    select_java,
};
use std::{process::Stdio, sync::Arc, time::Duration};
use tokio::io::{AsyncRead, AsyncReadExt};

const OUTPUT_LIMIT: usize = 256 * 1024;
const TERMINATION_TIMEOUT: Duration = Duration::from_secs(5);
const TRUNCATION_MARKER: &str = "\n[graphene: output truncated]\n";

#[derive(Clone)]
pub(crate) struct ServiceInstallToolRunner {
    context: Arc<ServiceContext>,
}

impl std::fmt::Debug for ServiceInstallToolRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceInstallToolRunner")
            .finish_non_exhaustive()
    }
}

impl ServiceInstallToolRunner {
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        Self { context }
    }
}

impl InstallToolRunner for ServiceInstallToolRunner {
    fn select_java<'a>(
        &'a self,
        requirement: Option<&'a graphene_minecraft::MinecraftJavaRequirement>,
        cancellation: graphene_core::CancellationToken,
    ) -> ToolJavaFuture<'a> {
        Box::pin(async move {
            if cancellation.is_cancelled() {
                return Err(tool_error(
                    ErrorCode::LoaderProcessorCancelled,
                    "loader processor Java selection was cancelled",
                ));
            }

            let runtime = match requirement {
                Some(requirement) => JavaService::new(Arc::clone(&self.context))
                    .select_for_requirement(&JavaRequirement {
                        major_version: requirement.major_version,
                        component_hint: requirement.component_hint.clone(),
                    })
                    .await
                    .map_err(|source| {
                        tool_error(
                            ErrorCode::LoaderProcessorJavaUnavailable,
                            "no compatible local or committed managed Java runtime is available for the loader processor",
                        )
                            .with_source(source)
                            .with_context("required_major", requirement.major_version.to_string())
                    })?,
                None => select_runtime(None).await?,
            };

            if cancellation.is_cancelled() {
                return Err(tool_error(
                    ErrorCode::LoaderProcessorCancelled,
                    "loader processor Java selection was cancelled",
                ));
            }

            Ok(SelectedToolJava {
                executable: runtime.executable,
                major_version: runtime.major_version,
            })
        })
    }

    fn run(&self, request: ToolRunRequest) -> ToolRunnerFuture<'_> {
        Box::pin(async move { run_tool(request).await })
    }
}

async fn run_tool(request: ToolRunRequest) -> Result<ToolRunResult> {
    if request.cancellation.is_cancelled() {
        return Err(tool_error(
            ErrorCode::LoaderProcessorCancelled,
            "loader processor was cancelled before execution",
        ));
    }

    if request.processor_id.is_empty()
        || request.processor_id.len() > 128
        || request.processor_id.contains('\0')
        || request.arguments.iter().any(|value| value.contains('\0'))
    {
        return Err(tool_error(
            ErrorCode::LoaderProcessorUnsupported,
            "loader processor request is invalid",
        ));
    }

    let mut command = tokio::process::Command::new(&request.java.executable);
    if request.main_class.is_empty()
        || request.main_class.len() > 512
        || request.main_class.contains('\0')
        || request.main_class.chars().any(char::is_whitespace)
    {
        return Err(tool_error(
            ErrorCode::LoaderProcessorUnsupported,
            "loader processor main class is invalid",
        ));
    }

    let mut classpath = Vec::with_capacity(request.classpath.len() + 1);
    classpath.push(request.executable_jar.clone());
    classpath.extend(request.classpath.iter().cloned());

    let joined = std::env::join_paths(classpath).map_err(|source| {
        tool_error(
            ErrorCode::LoaderProcessorUnsupported,
            "loader processor classpath is invalid",
        )
        .with_source(source)
    })?;

    command.arg("-cp").arg(joined).arg(&request.main_class);
    command.args(&request.arguments);
    command.current_dir(&request.working_directory);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.kill_on_drop(true);
    command.env_clear();

    copy_minimal_environment(&mut command);

    let mut child = command.spawn().map_err(|source| {
        tool_error(
            ErrorCode::LoaderProcessorFailed,
            "failed to spawn loader processor with the selected Java runtime",
        )
        .with_source(source)
        .with_context("processor", request.processor_id.clone())
    })?;

    let stdout = child.stdout.take().ok_or_else(|| {
        tool_error(
            ErrorCode::LoaderProcessorFailed,
            "loader processor stdout pipe is unavailable",
        )
    })?;

    let stderr = child.stderr.take().ok_or_else(|| {
        tool_error(
            ErrorCode::LoaderProcessorFailed,
            "loader processor stderr pipe is unavailable",
        )
    })?;

    let stdout_task = tokio::spawn(capture_output(stdout));
    let stderr_task = tokio::spawn(capture_output(stderr));
    let cancellation = request.cancellation.clone();

    enum Completion {
        Status(std::io::Result<std::process::ExitStatus>),
        Cancelled,
        TimedOut,
    }

    let completion = tokio::select! {
        status = child.wait() => Completion::Status(status),
        () = cancellation.cancelled() => Completion::Cancelled,
        () = tokio::time::sleep(request.timeout) => Completion::TimedOut,
    };

    match completion {
        Completion::Status(status) => {
            let status = status.map_err(|source| {
                tool_error(
                    ErrorCode::LoaderProcessorFailed,
                    "failed while waiting for loader processor",
                )
                .with_source(source)
            })?;
            let stdout = join_capture(stdout_task).await?;
            let stderr = join_capture(stderr_task).await?;
            Ok(ToolRunResult {
                exit_code: status.code(),
                stdout: stdout.text,
                stderr: stderr.text,
                stdout_truncated: stdout.truncated,
                stderr_truncated: stderr.truncated,
            })
        }

        Completion::Cancelled => {
            terminate_child(&mut child).await;
            let _ = join_capture(stdout_task).await;
            let _ = join_capture(stderr_task).await;
            Err(tool_error(
                ErrorCode::LoaderProcessorCancelled,
                "loader processor was cancelled",
            )
            .with_context("processor", request.processor_id))
        }

        Completion::TimedOut => {
            terminate_child(&mut child).await;
            let _ = join_capture(stdout_task).await;
            let _ = join_capture(stderr_task).await;
            Err(tool_error(
                ErrorCode::LoaderProcessorTimeout,
                "loader processor exceeded its bounded timeout",
            )
            .with_context("processor", request.processor_id))
        }
    }
}

async fn select_runtime(
    requirement: Option<&graphene_minecraft::MinecraftJavaRequirement>,
) -> Result<JavaRuntime> {
    if let Some(requirement) = requirement {
        let requirement = JavaRequirement {
            major_version: requirement.major_version,
            component_hint: requirement.component_hint.clone(),
        };

        return select_java(&requirement, None).await.map_err(|source| {
            tool_error(
                ErrorCode::LoaderProcessorJavaUnavailable,
                "no compatible local Java runtime is available for the loader processor",
            )
            .with_source(source)
            .with_context("required_major", requirement.major_version.to_string())
        });
    }

    let target = JavaArchitecture::current();
    for candidate in discover_java_candidates(None).await.map_err(|source| {
        tool_error(
            ErrorCode::LoaderProcessorJavaUnavailable,
            "failed to discover local Java runtimes for the loader processor",
        )
        .with_source(source)
    })? {
        if let Ok(runtime) = probe_java(&candidate).await
            && (target == JavaArchitecture::Other || runtime.architecture == target)
        {
            return Ok(runtime);
        }
    }

    Err(tool_error(
        ErrorCode::LoaderProcessorJavaUnavailable,
        "no usable local Java runtime is available for the loader processor",
    ))
}

fn copy_minimal_environment(command: &mut tokio::process::Command) {
    for name in [
        "PATH",
        "SystemRoot",
        "WINDIR",
        "TEMP",
        "TMP",
        "HOME",
        "USERPROFILE",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
}

struct CapturedOutput {
    text: String,
    truncated: bool,
}

async fn capture_output(mut reader: impl AsyncRead + Unpin) -> std::io::Result<CapturedOutput> {
    let mut captured = Vec::with_capacity(OUTPUT_LIMIT.min(32 * 1024));
    let mut buffer = [0_u8; 16 * 1024];
    let mut truncated = false;

    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }

        if captured.len() < OUTPUT_LIMIT {
            let remaining = OUTPUT_LIMIT - captured.len();
            let take = read.min(remaining);
            captured.extend_from_slice(&buffer[..take]);
            if take < read {
                truncated = true;
            }
        } else {
            truncated = true;
        }
    }

    let mut text = String::from_utf8_lossy(&captured).into_owned();
    if truncated {
        text.push_str(TRUNCATION_MARKER);
    }

    Ok(CapturedOutput { text, truncated })
}

async fn join_capture(
    task: tokio::task::JoinHandle<std::io::Result<CapturedOutput>>,
) -> Result<CapturedOutput> {
    task.await
        .map_err(|source| {
            tool_error(
                ErrorCode::LoaderProcessorFailed,
                "loader processor output drain task failed",
            )
            .with_source(source)
        })?
        .map_err(|source| {
            tool_error(
                ErrorCode::LoaderProcessorFailed,
                "failed to drain loader processor output",
            )
            .with_source(source)
        })
}

async fn terminate_child(child: &mut tokio::process::Child) {
    let _ = child.start_kill();
    let _ = tokio::time::timeout(TERMINATION_TIMEOUT, child.wait()).await;
}

fn tool_error(code: ErrorCode, message: &'static str) -> GrapheneError {
    GrapheneError::new(code, ErrorKind::Install, message)
}

#[cfg(test)]
mod tests;
