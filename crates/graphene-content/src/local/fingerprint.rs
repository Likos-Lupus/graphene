use graphene_core::{
    CancellationToken, ErrorCode, ErrorKind, GrapheneError, Result, Sha1Digest, Sha256Digest,
};
use graphene_instance::ManagedRelativePath;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::{fmt, fs::File, io::Read, path::Path};

/// Computes the CurseForge-compatible normalized Murmur2 fingerprint.
///
/// Strips ASCII whitespace characters (0x09 '\t', 0x0A '\n', 0x0D '\r', 0x20 ' ')
/// and applies Murmur2 with seed 1.
#[must_use]
pub fn compute_murmur2(bytes: &[u8]) -> u32 {
    let filtered: Vec<u8> = bytes
        .iter()
        .copied()
        .filter(|&b| b != 0x09 && b != 0x0A && b != 0x0D && b != 0x20)
        .collect();
    murmur2_32(&filtered, 1)
}

fn murmur2_32(bytes: &[u8], seed: u32) -> u32 {
    const M: u32 = 0x5bd1_e995;
    const R: u32 = 24;

    let len = bytes.len() as u32;
    let mut h = seed ^ len;

    let full_len = bytes.len() - (bytes.len() % 4);
    let mut index = 0;
    while index < full_len {
        let chunk = [
            bytes[index],
            bytes[index + 1],
            bytes[index + 2],
            bytes[index + 3],
        ];
        let mut k = u32::from_le_bytes(chunk);
        k = k.wrapping_mul(M);
        k ^= k >> R;
        k = k.wrapping_mul(M);

        h = h.wrapping_mul(M);
        h ^= k;
        index += 4;
    }

    let remainder = &bytes[full_len..];
    if remainder.len() == 3 {
        h ^= u32::from(remainder[2]) << 16;
    }
    if remainder.len() >= 2 {
        h ^= u32::from(remainder[1]) << 8;
    }
    if !remainder.is_empty() {
        h ^= u32::from(remainder[0]);
        h = h.wrapping_mul(M);
    }

    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^= h >> 15;

    h
}

/// Computes SHA-1 and SHA-256 in a single streaming pass over an open file.
pub fn stream_file_hashes(
    path: &Path,
    cancellation: &CancellationToken,
) -> Result<(Sha1Digest, Sha256Digest, u64)> {
    let mut file = File::open(path).map_err(|source| {
        GrapheneError::new(
            ErrorCode::FileOpenFailed,
            ErrorKind::Filesystem,
            format!("failed to open file for hashing: {}", path.display()),
        )
        .with_source(source)
    })?;

    let mut sha1 = Sha1::new();
    let mut sha256 = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut total_bytes = 0u64;

    loop {
        if cancellation.is_cancelled() {
            return Err(GrapheneError::new(
                ErrorCode::OperationCancelled,
                ErrorKind::Cancelled,
                "file hashing was cancelled",
            ));
        }

        let bytes_read = file.read(&mut buffer).map_err(|source| {
            GrapheneError::new(
                ErrorCode::FileOpenFailed,
                ErrorKind::Filesystem,
                format!("failed to read file during hashing: {}", path.display()),
            )
            .with_source(source)
        })?;

        if bytes_read == 0 {
            break;
        }

        let chunk = &buffer[..bytes_read];
        sha1.update(chunk);
        sha256.update(chunk);
        total_bytes += bytes_read as u64;
    }

    let sha1_bytes: [u8; 20] = sha1.finalize().into();
    let sha256_bytes: [u8; 32] = sha256.finalize().into();

    Ok((
        Sha1Digest::from_bytes(sha1_bytes),
        Sha256Digest::from_bytes(sha256_bytes),
        total_bytes,
    ))
}

/// Deterministic fingerprint of the physical mod inventory.
///
/// Used in conjunction with `InstanceStateFingerprint` to detect physical inventory changes
/// (e.g. manually added/removed/modified JARs).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentInventoryFingerprint(Sha256Digest);

impl ContentInventoryFingerprint {
    #[must_use]
    pub const fn from_digest(digest: Sha256Digest) -> Self {
        Self(digest)
    }

    /// Computes the deterministic inventory fingerprint from sorted file entries.
    #[must_use]
    pub fn compute<'a>(
        entries: impl IntoIterator<
            Item = (&'a ManagedRelativePath, bool, u64, Option<&'a Sha256Digest>),
        >,
    ) -> Self {
        let mut hasher = Sha256::new();
        for (rel_path, enabled, size, sha256) in entries {
            hasher.update(rel_path.as_str().as_bytes());
            hasher.update(if enabled { [1u8] } else { [0u8] });
            hasher.update(size.to_be_bytes());
            if let Some(digest) = sha256 {
                hasher.update(digest.as_bytes());
            }
        }
        let bytes: [u8; 32] = hasher.finalize().into();
        Self(Sha256Digest::from_bytes(bytes))
    }

    #[must_use]
    pub fn digest(&self) -> &Sha256Digest {
        &self.0
    }
}

impl fmt::Display for ContentInventoryFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Debug for ContentInventoryFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ContentInventoryFingerprint")
            .field(&self.0.to_string())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn murmur2_strips_whitespace() {
        let text_with_spaces = b"hello   world \n \t \r ";
        let text_without = b"helloworld";
        assert_eq!(
            compute_murmur2(text_with_spaces),
            compute_murmur2(text_without)
        );
    }

    #[test]
    fn murmur2_matches_reference_vectors() {
        // Vectors generated from the canonical Austin Appleby MurmurHash2 x86_32
        // reference implementation with seed 1, as required by CurseForge.
        assert_eq!(compute_murmur2(b""), 0x5bd1_5e36);
        assert_eq!(compute_murmur2(b"hello world"), 0xa85c_bded);
        assert_eq!(compute_murmur2(b"helloworld"), 0xa85c_bded);
        assert_eq!(compute_murmur2(b"a"), 0x2550_b18c);
    }
}
