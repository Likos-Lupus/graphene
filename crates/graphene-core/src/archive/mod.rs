//! Bounded, filesystem-neutral ZIP/DEFLATE primitives shared by trusted callers.
//!
//! This module deliberately owns only archive byte parsing and decompression. Callers retain
//! responsibility for domain-specific resource policy, path containment, publication, and error
//! classification.

mod deflate;
mod file_reader;
mod writer;
mod zip;

use std::{error::Error, fmt};

pub use deflate::{RawDeflateReader, inflate_raw_bounded};
pub use file_reader::ArchiveFile;
pub use writer::{DeterministicZipWriter, ZipWriterEntry};
pub use zip::{ZipEntry, central_entries, crc32, extract_entry};

/// Error category produced by the low-level archive codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ArchiveCodecErrorKind {
    Invalid,
    Cancelled,
    Io,
}

/// Filesystem-neutral error from bounded archive byte parsing/decompression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveCodecError {
    kind: ArchiveCodecErrorKind,
    message: &'static str,
}

impl ArchiveCodecError {
    #[must_use]
    pub const fn invalid(message: &'static str) -> Self {
        Self {
            kind: ArchiveCodecErrorKind::Invalid,
            message,
        }
    }

    #[must_use]
    pub const fn cancelled() -> Self {
        Self {
            kind: ArchiveCodecErrorKind::Cancelled,
            message: "archive operation was cancelled",
        }
    }

    #[must_use]
    pub const fn io(message: &'static str) -> Self {
        Self {
            kind: ArchiveCodecErrorKind::Io,
            message,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> ArchiveCodecErrorKind {
        self.kind
    }

    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        matches!(self.kind, ArchiveCodecErrorKind::Cancelled)
    }
}

impl fmt::Display for ArchiveCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}

impl Error for ArchiveCodecError {}

pub type ArchiveCodecResult<T> = std::result::Result<T, ArchiveCodecError>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CancellationToken;

    #[test]
    fn crc32_matches_known_vector() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn archive_codec_bounds_are_fail_closed() {
        assert!(central_entries(&[], 1, 1).is_err());
        assert!(inflate_raw_bounded(&[], 1, 0, &CancellationToken::new()).is_err());
    }
}
