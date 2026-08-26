use super::limits::{
    MAX_ARCHIVE_ENTRIES, MAX_ENTRY_NAME_BYTES, MAX_EXPANDED_ENTRY_BYTES, MAX_TOTAL_EXPANSION_BYTES,
};
use crate::error::PackError;
use graphene_core::CancellationToken;
use graphene_core::archive::{ArchiveCodecError, ArchiveFile, ZipEntry};
use graphene_core::{ErrorCode, Sha256Digest};
use sha2::{Digest, Sha256};
use std::io::{Read, Seek};

/// Bounded view over one pack archive backed by a reader.
///
/// The index never requires the whole archive in memory: central-directory parsing and entry
/// streaming go through the centralized `graphene-core::archive` reader boundary.
pub struct PackArchiveIndex<R: Read + Seek> {
    inner: ArchiveFile<R>,
    total_expansion_budget: u64,
    expanded_so_far: u64,
}

impl<R: Read + Seek> PackArchiveIndex<R> {
    /// Opens the archive under the shared import limits.
    pub fn open(reader: R, cancellation: &CancellationToken) -> Result<Self, PackError> {
        let inner = ArchiveFile::open(
            reader,
            MAX_ARCHIVE_ENTRIES,
            MAX_ENTRY_NAME_BYTES,
            cancellation,
        )
        .map_err(codec)?;
        Ok(Self {
            inner,
            total_expansion_budget: MAX_TOTAL_EXPANSION_BYTES,
            expanded_so_far: 0,
        })
    }

    /// All indexed entries in central-directory order.
    #[must_use]
    pub fn entries(&self) -> &[ZipEntry] {
        self.inner.entries()
    }

    /// Entry lookup by exact archive name.
    #[must_use]
    pub fn entry(&self, name: &str) -> Option<&ZipEntry> {
        self.inner.entry(name)
    }

    /// Reads one manifest-class entry fully into memory under the manifest byte bound.
    pub fn read_manifest(&mut self, name: &str) -> Result<Vec<u8>, PackError> {
        let entry = self
            .inner
            .entry(name)
            .ok_or_else(|| PackError::archive("required archive entry is absent"))?
            .clone();
        if !entry.is_regular || entry.is_directory || entry.is_symlink {
            return Err(PackError::archive("manifest entry is not an ordinary file"));
        }
        if entry.uncompressed_size > super::limits::MAX_MANIFEST_BYTES {
            return Err(PackError::new(
                ErrorCode::PackSourceTooLarge,
                "manifest exceeds the size bound",
            ));
        }
        self.inner
            .read_entry(
                &entry,
                super::limits::MAX_MANIFEST_BYTES,
                &CancellationToken::new(),
            )
            .map_err(codec)
    }

    /// Streams one regular-file entry to `writer`, enforcing per-entry and cumulative expansion
    /// bounds plus CRC, while returning the observed SHA-256 of the payload.
    pub fn stream_entry_hashed(
        &mut self,
        name: &str,
        writer: &mut dyn std::io::Write,
        cancellation: &CancellationToken,
    ) -> Result<StreamedEntry, PackError> {
        let entry = self
            .inner
            .entry(name)
            .ok_or_else(|| PackError::archive("archive entry is absent"))?
            .clone();
        if !entry.is_regular {
            return Err(PackError::archive(
                "special archive entries cannot be extracted",
            ));
        }

        let expected = entry.uncompressed_size as u64;
        if expected > MAX_EXPANDED_ENTRY_BYTES {
            return Err(PackError::new(
                ErrorCode::PackSourceTooLarge,
                "expanded entry exceeds the single-entry bound",
            ));
        }

        if self.expanded_so_far.saturating_add(expected) > self.total_expansion_budget {
            return Err(PackError::new(
                ErrorCode::PackSourceTooLarge,
                "total expanded payload exceeds the operation bound",
            ));
        }

        let mut hasher = Sha256::new();
        let written = {
            let mut hashing = HashingForward {
                inner: writer,
                hasher: &mut hasher,
            };
            let count = self
                .inner
                .stream_entry_to(&entry, MAX_EXPANDED_ENTRY_BYTES, &mut hashing, cancellation)
                .map_err(codec)?;
            self.expanded_so_far += count;
            count
        };

        let digest_bytes: [u8; 32] = hasher.finalize().into();
        Ok(StreamedEntry {
            size: written,
            sha256: Sha256Digest::from_bytes(digest_bytes),
        })
    }

    /// Remaining expansion budget after prior streaming.
    #[must_use]
    pub const fn expanded_so_far(&self) -> u64 {
        self.expanded_so_far
    }
}

/// Observed facts about one streamed payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamedEntry {
    pub size: u64,
    pub sha256: Sha256Digest,
}

struct HashingForward<'a> {
    inner: &'a mut dyn std::io::Write,
    hasher: &'a mut Sha256,
}

impl std::io::Write for HashingForward<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.hasher.update(buf);
        self.inner.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn codec(error: ArchiveCodecError) -> PackError {
    if error.is_cancelled() {
        PackError::new(ErrorCode::OperationCancelled, error.to_string())
    } else {
        PackError::archive(error.to_string())
    }
}
