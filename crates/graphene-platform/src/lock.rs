use fs2::FileExt;
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use std::{fs::File, io::ErrorKind as IoErrorKind};

/// Advisory lock mode for cross-process synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdvisoryLockMode {
    /// Shared lock: multiple readers/launches can hold concurrently.
    Shared,
    /// Exclusive lock: exactly one writer/mutator can hold.
    Exclusive,
}

/// Attempts to acquire an advisory lock on an open file handle without blocking.
///
/// Returns:
/// - `Ok(true)` if the lock was successfully acquired.
/// - `Ok(false)` if the lock could not be acquired because another process holds a conflicting lock.
/// - `Err(GrapheneError)` on unexpected I/O failure.
pub fn try_lock_advisory(file: &File, mode: AdvisoryLockMode) -> Result<bool> {
    let result = match mode {
        AdvisoryLockMode::Shared => FileExt::try_lock_shared(file),
        AdvisoryLockMode::Exclusive => FileExt::try_lock_exclusive(file),
    };

    match result {
        Ok(()) => Ok(true),
        Err(error) if is_lock_contended(&error) => Ok(false),
        Err(source) => Err(GrapheneError::new(
            ErrorCode::PlatformUnsupported,
            ErrorKind::Platform,
            "failed to acquire advisory file lock",
        )
        .with_source(source)),
    }
}

/// Acquires an advisory lock on an open file handle, blocking until acquired.
pub fn lock_advisory(file: &File, mode: AdvisoryLockMode) -> Result<()> {
    let result = match mode {
        AdvisoryLockMode::Shared => FileExt::lock_shared(file),
        AdvisoryLockMode::Exclusive => FileExt::lock_exclusive(file),
    };

    result.map_err(|source| {
        GrapheneError::new(
            ErrorCode::PlatformUnsupported,
            ErrorKind::Platform,
            "failed to acquire advisory file lock",
        )
        .with_source(source)
    })
}

/// Releases an advisory lock on an open file handle.
pub fn unlock_advisory(file: &File) -> Result<()> {
    FileExt::unlock(file).map_err(|source| {
        GrapheneError::new(
            ErrorCode::PlatformUnsupported,
            ErrorKind::Platform,
            "failed to release advisory file lock",
        )
        .with_source(source)
    })
}

/// RAII guard holding an active advisory lock on an open file.
///
/// Unlocks the file automatically on drop.
#[derive(Debug)]
pub struct AdvisoryGuard {
    file: File,
    mode: AdvisoryLockMode,
}

impl AdvisoryGuard {
    /// Wraps an already-locked file in an RAII guard.
    #[must_use]
    pub fn new(file: File, mode: AdvisoryLockMode) -> Self {
        Self { file, mode }
    }

    /// Attempts to acquire an advisory lock on `file`, returning a guard if acquired.
    ///
    /// If contended, returns `Ok(None)`.
    pub fn try_acquire(file: File, mode: AdvisoryLockMode) -> Result<Option<Self>> {
        if try_lock_advisory(&file, mode)? {
            Ok(Some(Self::new(file, mode)))
        } else {
            Ok(None)
        }
    }

    /// Acquires an advisory lock on `file`, blocking until acquired.
    pub fn acquire(file: File, mode: AdvisoryLockMode) -> Result<Self> {
        lock_advisory(&file, mode)?;
        Ok(Self::new(file, mode))
    }

    /// Returns a reference to the underlying file.
    #[must_use]
    pub fn file(&self) -> &File {
        &self.file
    }

    /// Returns the lock mode.
    #[must_use]
    pub fn mode(&self) -> AdvisoryLockMode {
        self.mode
    }
}

impl Drop for AdvisoryGuard {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

fn is_lock_contended(error: &std::io::Error) -> bool {
    if error.kind() == IoErrorKind::WouldBlock {
        return true;
    }
    #[cfg(windows)]
    if let Some(code) = error.raw_os_error() {
        return code == 33 || code == 32;
    }
    #[cfg(unix)]
    if let Some(code) = error.raw_os_error() {
        return code == libc::EAGAIN || code == libc::EWOULDBLOCK;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;

    #[test]
    fn shared_and_exclusive_locking_semantics() {
        let temp = tempfile::tempdir().unwrap();
        let lock_path = temp.path().join("test.lock");

        let file1 = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .unwrap();

        let file2 = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .unwrap();

        // Shared lock file1
        let guard1 = AdvisoryGuard::try_acquire(file1, AdvisoryLockMode::Shared)
            .unwrap()
            .expect("should acquire shared lock");

        // Shared lock file2 concurrently should succeed
        let guard2 = AdvisoryGuard::try_acquire(file2, AdvisoryLockMode::Shared)
            .unwrap()
            .expect("should acquire second shared lock");

        drop(guard1);
        drop(guard2);

        // Exclusive lock file1
        let file1 = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        let guard_ex = AdvisoryGuard::try_acquire(file1, AdvisoryLockMode::Exclusive)
            .unwrap()
            .expect("should acquire exclusive lock");

        // Shared lock file2 while exclusive is held must fail fast
        let file2 = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        let guard_sh = AdvisoryGuard::try_acquire(file2, AdvisoryLockMode::Shared).unwrap();
        assert!(
            guard_sh.is_none(),
            "shared lock must fail when exclusive is held"
        );

        // Exclusive lock file3 while exclusive is held must fail fast
        let file3 = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        let guard_ex2 = AdvisoryGuard::try_acquire(file3, AdvisoryLockMode::Exclusive).unwrap();
        assert!(
            guard_ex2.is_none(),
            "exclusive lock must fail when exclusive is held"
        );

        drop(guard_ex);

        // After dropping exclusive guard, file3 can acquire exclusive
        let file3 = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        let guard_ex3 = AdvisoryGuard::try_acquire(file3, AdvisoryLockMode::Exclusive).unwrap();
        assert!(
            guard_ex3.is_some(),
            "exclusive lock should succeed after release"
        );
    }
}
