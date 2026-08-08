use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    io,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::AsyncReadExt,
    process::{Child, ChildStderr, ChildStdout, Command},
    time,
};

/// Direct process specification. Arguments are always passed as an argv vector; no shell is used.
#[derive(Clone)]
pub struct ProcessSpec {
    executable: PathBuf,
    args: Vec<OsString>,
    current_dir: Option<PathBuf>,
    env: BTreeMap<OsString, OsString>,
    clear_env: bool,
}

impl std::fmt::Debug for ProcessSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessSpec")
            .field("executable", &self.executable)
            .field("arg_count", &self.args.len())
            .field("current_dir", &self.current_dir)
            .field("environment_keys", &self.env.keys().collect::<Vec<_>>())
            .field("clear_env", &self.clear_env)
            .finish()
    }
}

impl ProcessSpec {
    #[must_use]
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            args: Vec::new(),
            current_dir: None,
            env: BTreeMap::new(),
            clear_env: false,
        }
    }

    #[must_use]
    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    #[must_use]
    pub fn current_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.current_dir = Some(path.into());
        self
    }

    #[must_use]
    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    #[must_use]
    pub const fn clear_env(mut self, clear: bool) -> Self {
        self.clear_env = clear;
        self
    }

    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    #[must_use]
    pub fn argv(&self) -> &[OsString] {
        &self.args
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.executable);
        command.args(&self.args);
        if let Some(current_dir) = &self.current_dir {
            command.current_dir(current_dir);
        }
        if self.clear_env {
            command.env_clear();
        }
        command.envs(&self.env);
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        command.kill_on_drop(true);
        command
    }
}

/// Opaque stdout/stderr pipe that keeps Tokio child stream types private.
pub struct ProcessOutput {
    inner: ProcessOutputInner,
}

enum ProcessOutputInner {
    Stdout(ChildStdout),
    Stderr(ChildStderr),
}

impl ProcessOutput {
    /// Reads at most `buffer.len()` bytes. Invalid UTF-8 policy belongs to the caller.
    pub async fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match &mut self.inner {
            ProcessOutputInner::Stdout(stream) => stream.read(buffer).await,
            ProcessOutputInner::Stderr(stream) => stream.read(buffer).await,
        }
    }
}

/// Graphene-owned process-exit facts without exposing `std::process::ExitStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessExit {
    pub success: bool,
    pub code: Option<i32>,
}

/// Direct child-process owner. Tokio's child type remains private to this crate.
pub struct PlatformProcess {
    child: Child,
}

impl PlatformProcess {
    pub fn spawn(spec: &ProcessSpec) -> io::Result<Self> {
        let child = spec.command().spawn()?;
        Ok(Self { child })
    }

    #[must_use]
    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    pub fn take_stdout(&mut self) -> Option<ProcessOutput> {
        self.child.stdout.take().map(|inner| ProcessOutput {
            inner: ProcessOutputInner::Stdout(inner),
        })
    }

    pub fn take_stderr(&mut self) -> Option<ProcessOutput> {
        self.child.stderr.take().map(|inner| ProcessOutput {
            inner: ProcessOutputInner::Stderr(inner),
        })
    }

    pub async fn wait(&mut self) -> io::Result<ProcessExit> {
        let status = self.child.wait().await?;
        Ok(ProcessExit {
            success: status.success(),
            code: status.code(),
        })
    }

    pub async fn kill(&mut self) -> io::Result<()> {
        self.child.kill().await
    }
}

/// Bounded captured output for short-lived probe/helper processes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedProcess {
    pub timed_out: bool,
    pub exit: Option<ProcessExit>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

/// Runs a direct process with a hard timeout and independent stdout/stderr byte caps.
pub async fn run_process_bounded(
    spec: &ProcessSpec,
    timeout: Duration,
    max_stream_bytes: usize,
) -> io::Result<CapturedProcess> {
    let mut process = PlatformProcess::spawn(spec)?;
    let stdout = process.take_stdout();
    let stderr = process.take_stderr();
    let stdout_task = tokio::spawn(capture(stdout, max_stream_bytes));
    let stderr_task = tokio::spawn(capture(stderr, max_stream_bytes));

    let (timed_out, exit) = match time::timeout(timeout, process.wait()).await {
        Ok(result) => (false, Some(result?)),
        Err(_) => {
            let _ = process.kill().await;
            let exit = process.wait().await.ok();
            (true, exit)
        }
    };

    let (stdout, stdout_truncated) = stdout_task.await.map_err(join_error)??;
    let (stderr, stderr_truncated) = stderr_task.await.map_err(join_error)??;
    Ok(CapturedProcess {
        timed_out,
        exit,
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    })
}

async fn capture(stream: Option<ProcessOutput>, limit: usize) -> io::Result<(Vec<u8>, bool)> {
    let Some(mut stream) = stream else {
        return Ok((Vec::new(), false));
    };
    let mut captured = Vec::with_capacity(limit.min(8192));
    let mut truncated = false;
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(captured.len());
        let keep = remaining.min(read);
        captured.extend_from_slice(&buffer[..keep]);
        if keep < read {
            truncated = true;
        }
    }
    Ok((captured, truncated))
}

fn join_error(error: tokio::task::JoinError) -> io::Error {
    io::Error::other(format!("process output task failed: {error}"))
}

/// Current platform's Java classpath separator.
#[must_use]
pub const fn classpath_separator() -> char {
    if cfg!(windows) { ';' } else { ':' }
}

/// Returns whether an executable path is plausibly usable before a spawn attempt.
#[must_use]
pub fn executable_exists(path: &Path) -> bool {
    path.is_file()
}

/// Converts an OS argument for diagnostic-safe length checks without requiring UTF-8.
#[must_use]
pub fn os_arg_is_empty(value: &OsStr) -> bool {
    value.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classpath_separator_matches_target() {
        if cfg!(windows) {
            assert_eq!(classpath_separator(), ';');
        } else {
            assert_eq!(classpath_separator(), ':');
        }
    }
}
