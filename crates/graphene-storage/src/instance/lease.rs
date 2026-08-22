use crate::instance::paths::InstancePaths;
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, InstanceId, Result};
use graphene_platform::{AdvisoryGuard, AdvisoryLockMode};
use std::{
    any::Any,
    fs::{self, File, OpenOptions},
};

/// RAII token for a shared instance lease.
#[derive(Debug)]
pub struct InstanceSharedLease {
    instance_id: InstanceId,
    _guard: AdvisoryGuard,
}

impl InstanceSharedLease {
    /// Returns the leased instance identifier.
    #[must_use]
    pub fn instance_id(&self) -> InstanceId {
        self.instance_id
    }

    /// Converts this lease into a type-erased RAII token suitable for transfer across crate
    /// boundaries without leaking storage types.
    #[must_use]
    pub fn into_opaque(self) -> Box<dyn Any + Send + Sync> {
        Box::new(self)
    }
}

/// RAII token for an exclusive instance lease.
#[derive(Debug)]
pub struct InstanceExclusiveLease {
    instance_id: InstanceId,
    _guard: AdvisoryGuard,
}

impl InstanceExclusiveLease {
    /// Returns the leased instance identifier.
    #[must_use]
    pub fn instance_id(&self) -> InstanceId {
        self.instance_id
    }

    /// Converts this lease into a type-erased RAII token suitable for transfer across crate
    /// boundaries.
    #[must_use]
    pub fn into_opaque(self) -> Box<dyn Any + Send + Sync> {
        Box::new(self)
    }
}

/// Store for managing persistent advisory lock carriers and leases.
#[derive(Debug, Clone)]
pub struct InstanceLeaseStore {
    paths: InstancePaths,
}

impl InstanceLeaseStore {
    /// Creates a new lease store using the given instance paths.
    #[must_use]
    pub fn new(paths: InstancePaths) -> Self {
        Self { paths }
    }

    /// Acquires a shared advisory lease on `id`, failing fast with `ErrorCode::InstanceBusy` if an
    /// exclusive lease is already held.
    pub fn acquire_shared(&self, id: InstanceId) -> Result<InstanceSharedLease> {
        let file = self.open_lock_carrier(id)?;
        let guard =
            AdvisoryGuard::try_acquire(file, AdvisoryLockMode::Shared)?.ok_or_else(|| {
                GrapheneError::new(
                    ErrorCode::InstanceBusy,
                    ErrorKind::Instance,
                    "instance is busy (shared lease contested by exclusive operation)",
                )
                .with_context("instance_id", id.to_string())
            })?;

        Ok(InstanceSharedLease {
            instance_id: id,
            _guard: guard,
        })
    }

    /// Acquires an exclusive advisory lease on `id`, failing fast with `ErrorCode::InstanceBusy` if
    /// any shared or exclusive lease is already held.
    pub fn acquire_exclusive(&self, id: InstanceId) -> Result<InstanceExclusiveLease> {
        let file = self.open_lock_carrier(id)?;
        let guard =
            AdvisoryGuard::try_acquire(file, AdvisoryLockMode::Exclusive)?.ok_or_else(|| {
                GrapheneError::new(
                ErrorCode::InstanceBusy,
                ErrorKind::Instance,
                "instance is busy (exclusive lease contested by active operation or running game)",
            )
            .with_context("instance_id", id.to_string())
            })?;

        Ok(InstanceExclusiveLease {
            instance_id: id,
            _guard: guard,
        })
    }

    /// Acquires exclusive leases on two instances in deterministic lexicographical order to prevent
    /// deadlocks across concurrent clone or migration operations.
    pub fn acquire_exclusive_pair(
        &self,
        id1: InstanceId,
        id2: InstanceId,
    ) -> Result<(InstanceExclusiveLease, InstanceExclusiveLease)> {
        if id1 == id2 {
            return Err(GrapheneError::new(
                ErrorCode::InstanceInvalid,
                ErrorKind::Instance,
                "cannot acquire exclusive lease pair on identical instance id",
            )
            .with_context("instance_id", id1.to_string()));
        }

        if id1 < id2 {
            let lease1 = self.acquire_exclusive(id1)?;
            let lease2 = self.acquire_exclusive(id2)?;
            Ok((lease1, lease2))
        } else {
            let lease2 = self.acquire_exclusive(id2)?;
            let lease1 = self.acquire_exclusive(id1)?;
            Ok((lease1, lease2))
        }
    }

    fn open_lock_carrier(&self, id: InstanceId) -> Result<File> {
        let locks_dir = self.paths.locks_dir();
        fs::create_dir_all(&locks_dir).map_err(|source| {
            GrapheneError::new(
                ErrorCode::DirectoryCreateFailed,
                ErrorKind::Filesystem,
                "failed to create instance locks directory",
            )
            .with_context("path", locks_dir.display().to_string())
            .with_source(source)
        })?;

        let carrier_path = self.paths.lock_carrier_path(id);
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&carrier_path)
            .map_err(|source| {
                GrapheneError::new(
                    ErrorCode::FileOpenFailed,
                    ErrorKind::Filesystem,
                    "failed to open persistent instance lock carrier file",
                )
                .with_context("path", carrier_path.display().to_string())
                .with_source(source)
            })
    }
}
