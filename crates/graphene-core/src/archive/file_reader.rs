use super::{ArchiveCodecError, ArchiveCodecResult, RawDeflateReader, ZipEntry};
use crate::CancellationToken;
use std::io::{Read, Seek, SeekFrom, Write};

/// Bounded file-backed ZIP archive reader.
///
/// Unlike the in-memory byte codec, this reader parses the end record and central directory via
/// [`Read`] + [`Seek`] and streams selected entry payloads without ever requiring the whole
/// archive to be resident in memory. ZIP64, multi-disk, and encrypted archives are rejected.
pub struct ArchiveFile<R: Read + Seek> {
    inner: R,
    entries: Vec<ZipEntry>,
}

const EOCD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
const LOCAL_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x03, 0x04];
const CENTRAL_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];

impl<R: Read + Seek> ArchiveFile<R> {
    /// Opens an archive, parsing its central directory under the supplied bounds.
    pub fn open(
        mut reader: R,
        max_entries: usize,
        max_name_bytes: usize,
        cancellation: &CancellationToken,
    ) -> ArchiveCodecResult<Self> {
        let eocd_offset = find_eocd(&mut reader, cancellation)?;
        let tail = read_exact_at(&mut reader, eocd_offset, 22)?;

        let disk = u16_le(&tail[4..6]);
        let central_disk = u16_le(&tail[6..8]);
        let disk_entries = u16_le(&tail[8..10]) as usize;
        let total_entries = u16_le(&tail[10..12]) as usize;
        let central_size = u32_le(&tail[12..16]) as usize;
        let central_offset = u32_le(&tail[16..20]) as usize;

        if disk != 0 || central_disk != 0 || disk_entries != total_entries {
            return Err(ArchiveCodecError::invalid(
                "multi-disk ZIP archives are unsupported",
            ));
        }

        if total_entries == u16::MAX as usize
            || central_size == u32::MAX as usize
            || central_offset == u32::MAX as usize
        {
            return Err(ArchiveCodecError::invalid("ZIP64 archives are unsupported"));
        }

        if total_entries > max_entries {
            return Err(ArchiveCodecError::invalid(
                "ZIP archive exceeds the configured entry count limit",
            ));
        }

        let mut entries = Vec::with_capacity(total_entries.min(4096));
        let mut cursor = central_offset as u64;
        for _ in 0..total_entries {
            if cancellation.is_cancelled() {
                return Err(ArchiveCodecError::cancelled());
            }
            let record = read_exact_at(&mut reader, cursor, 46)?;
            if record[..4] != CENTRAL_SIGNATURE {
                return Err(ArchiveCodecError::invalid(
                    "ZIP archive central directory entry is malformed",
                ));
            }

            let made_by = u16_le(&record[4..6]);
            let flags = u16_le(&record[8..10]);
            let method = u16_le(&record[10..12]);
            let crc = u32_le(&record[16..20]);
            let compressed_size = u32_le(&record[20..24]) as usize;
            let uncompressed_size = u32_le(&record[24..28]) as usize;
            let name_len = u16_le(&record[28..30]) as usize;
            let extra_len = u16_le(&record[30..32]) as usize;
            let comment_len = u16_le(&record[32..34]) as usize;
            let external = u32_le(&record[38..42]);
            let local_offset = u32_le(&record[42..46]) as usize;

            if compressed_size == u32::MAX as usize
                || uncompressed_size == u32::MAX as usize
                || local_offset == u32::MAX as usize
            {
                return Err(ArchiveCodecError::invalid(
                    "ZIP64 archive entry is unsupported",
                ));
            }

            if flags & 0x0001 != 0 {
                return Err(ArchiveCodecError::invalid(
                    "encrypted ZIP archive entries are unsupported",
                ));
            }

            if name_len == 0 || name_len > max_name_bytes {
                return Err(ArchiveCodecError::invalid(
                    "ZIP archive entry name length is invalid",
                ));
            }

            let name_buffer =
                read_exact_at(&mut reader, cursor + 46, name_len.min(max_name_bytes))?;
            let name = std::str::from_utf8(&name_buffer)
                .map_err(|_| ArchiveCodecError::invalid("ZIP archive entry name is not UTF-8"))?
                .to_owned();

            let host = made_by >> 8;
            let unix_mode = (external >> 16) as u16;
            let file_type = unix_mode & 0o170000;
            let is_directory = name.ends_with('/') || (host == 3 && file_type == 0o040000);
            let is_symlink = host == 3 && file_type == 0o120000;
            let is_regular = !is_directory
                && !is_symlink
                && (host != 3 || file_type == 0 || file_type == 0o100000);

            entries.push(ZipEntry {
                name,
                flags,
                method,
                crc32: crc,
                compressed_size,
                uncompressed_size,
                local_offset,
                is_directory,
                is_symlink,
                is_regular,
            });

            cursor = cursor
                .checked_add(46 + name_len as u64 + extra_len as u64 + comment_len as u64)
                .ok_or_else(|| {
                    ArchiveCodecError::invalid("ZIP archive central directory overflowed")
                })?;
        }

        Ok(Self {
            inner: reader,
            entries,
        })
    }

    /// Returns the parsed central-directory entries.
    #[must_use]
    pub fn entries(&self) -> &[ZipEntry] {
        &self.entries
    }

    /// Finds an entry by exact archive name.
    #[must_use]
    pub fn entry(&self, name: &str) -> Option<&ZipEntry> {
        self.entries.iter().find(|entry| entry.name == name)
    }

    /// Streams one entry payload to `writer` under a strict per-entry output bound, verifying
    /// declared size and CRC while streaming. Returns the number of bytes written.
    pub fn stream_entry_to(
        &mut self,
        entry: &ZipEntry,
        maximum_size: u64,
        writer: &mut dyn Write,
        cancellation: &CancellationToken,
    ) -> ArchiveCodecResult<u64> {
        if entry.is_directory {
            return Err(ArchiveCodecError::invalid(
                "directory archive entries cannot be streamed",
            ));
        }

        match entry.method {
            0 | 8 => {}
            _ => {
                return Err(ArchiveCodecError::invalid(
                    "ZIP archive uses an unsupported compression method",
                ));
            }
        }

        if u64::try_from(entry.uncompressed_size).is_ok_and(|size| size > maximum_size) {
            return Err(ArchiveCodecError::invalid(
                "ZIP archive entry exceeds its configured output bound",
            ));
        }

        let local = read_exact_at(&mut self.inner, entry.local_offset as u64, 30)?;
        if local[..4] != LOCAL_SIGNATURE {
            return Err(ArchiveCodecError::invalid(
                "ZIP archive local entry is malformed",
            ));
        }

        let local_flags = u16_le(&local[6..8]);
        let local_method = u16_le(&local[8..10]);
        if local_flags & 0x0001 != 0
            || local_method != entry.method
            || (local_flags & 0x0800) != (entry.flags & 0x0800)
        {
            return Err(ArchiveCodecError::invalid(
                "ZIP archive local/central entry metadata is inconsistent",
            ));
        }

        let name_len = u16_le(&local[26..28]) as u64;
        let extra_len = u16_le(&local[28..30]) as u64;
        let data_start = (entry.local_offset as u64)
            .checked_add(30)
            .and_then(|offset| offset.checked_add(name_len))
            .and_then(|offset| offset.checked_add(extra_len))
            .ok_or_else(|| {
                ArchiveCodecError::invalid("ZIP archive local entry offset overflowed")
            })?;

        self.inner
            .seek(SeekFrom::Start(data_start))
            .map_err(|_| ArchiveCodecError::io("failed to seek to archive entry data"))?;

        let limited = LimitReader {
            inner: &mut self.inner,
            remaining: entry.compressed_size as u64,
        };

        let mut verifier = CrcWriter {
            writer,
            crc: 0xffff_ffff_u32,
            written: 0,
            maximum_size,
            cancellation,
        };

        match entry.method {
            0 => copy_bounded(limited, &mut verifier, cancellation)?,
            8 => {
                let mut inflater =
                    RawDeflateReader::new(limited, maximum_size, cancellation.clone());
                copy_bounded(&mut inflater, &mut verifier, cancellation)?;
                if !inflater.is_finished() {
                    return Err(ArchiveCodecError::invalid(
                        "deflate stream ended before its final block",
                    ));
                }
            }
            _ => unreachable!("unsupported compression methods are rejected above"),
        }

        if verifier.written != entry.uncompressed_size as u64 {
            return Err(ArchiveCodecError::invalid(
                "ZIP archive entry decompressed to an unexpected size",
            ));
        }

        let expected_crc = verifier.crc ^ 0xffff_ffff_u32;
        if expected_crc != entry.crc32 {
            return Err(ArchiveCodecError::invalid(
                "ZIP archive entry failed its CRC check",
            ));
        }

        Ok(verifier.written)
    }

    /// Reads a selected entry fully into memory under a strict byte bound.
    pub fn read_entry(
        &mut self,
        entry: &ZipEntry,
        maximum_size: usize,
        cancellation: &CancellationToken,
    ) -> ArchiveCodecResult<Vec<u8>> {
        let mut buffer = CollectingWriter { inner: Vec::new() };
        self.stream_entry_to(entry, maximum_size as u64, &mut buffer, cancellation)?;
        Ok(buffer.inner)
    }
}

struct CollectingWriter {
    inner: Vec<u8>,
}

impl Write for CollectingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct LimitReader<'a, R: Read> {
    inner: &'a mut R,
    remaining: u64,
}

impl<R: Read> Read for LimitReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.remaining == 0 {
            return Ok(0);
        }
        let count = buf.len().min(self.remaining as usize).min(64 * 1024);
        let read = self.inner.read(&mut buf[..count])?;
        self.remaining -= read as u64;
        Ok(read)
    }
}

struct CrcWriter<'a> {
    writer: &'a mut dyn Write,
    crc: u32,
    written: u64,
    maximum_size: u64,
    cancellation: &'a CancellationToken,
}

impl Write for CrcWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.cancellation.is_cancelled() {
            return Err(io_failure(ArchiveCodecError::cancelled()));
        }
        if self.written + buf.len() as u64 > self.maximum_size {
            return Err(io_failure(ArchiveCodecError::invalid(
                "archive entry exceeded its configured output bound",
            )));
        }

        self.crc = crc32_incremental(self.crc, buf);
        self.writer.write_all(buf)?;
        self.written += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

fn copy_bounded(
    mut source: impl Pull,
    sink: &mut CrcWriter<'_>,
    cancellation: &CancellationToken,
) -> ArchiveCodecResult<()> {
    let mut chunk = vec![0_u8; 64 * 1024];
    loop {
        if cancellation.is_cancelled() {
            return Err(ArchiveCodecError::cancelled());
        }
        let read = source.pull(&mut chunk)?;
        if read == 0 {
            return Ok(());
        }
        sink.write_all(&chunk[..read])
            .map_err(|_| ArchiveCodecError::io("archive entry output could not be written"))?;
    }
}

/// Uniform pull interface over stored bytes and decoded DEFLATE output.
trait Pull {
    fn pull(&mut self, buf: &mut [u8]) -> ArchiveCodecResult<usize>;
}

impl<T: Read> Pull for T {
    fn pull(&mut self, buf: &mut [u8]) -> ArchiveCodecResult<usize> {
        self.read(buf).map_err(|error| {
            if error.kind() == std::io::ErrorKind::Interrupted {
                ArchiveCodecError::cancelled()
            } else {
                ArchiveCodecError::io("failed to read archive data")
            }
        })
    }
}

fn io_failure(error: ArchiveCodecError) -> std::io::Error {
    std::io::Error::other(error.to_string())
}

/// Incremental CRC-32 (IEEE) over streamed chunks.
#[must_use]
pub fn crc32_incremental(current: u32, bytes: &[u8]) -> u32 {
    let mut crc = current;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    crc
}

fn find_eocd<R: Read + Seek>(
    reader: &mut R,
    cancellation: &CancellationToken,
) -> ArchiveCodecResult<u64> {
    let end = reader
        .seek(SeekFrom::End(0))
        .map_err(|_| ArchiveCodecError::io("failed to determine archive size"))?;
    if end < 22 {
        return Err(ArchiveCodecError::invalid(
            "ZIP archive has no valid end record",
        ));
    }

    let window_start = end.saturating_sub(22 + u64::from(u16::MAX));
    let window_len = (end - window_start) as usize;
    reader
        .seek(SeekFrom::Start(window_start))
        .map_err(|_| ArchiveCodecError::io("failed to inspect archive end record"))?;
    let mut window = vec![0_u8; window_len];
    reader
        .read_exact(&mut window)
        .map_err(|_| ArchiveCodecError::io("failed to read archive end record"))?;

    for offset in (0..=window_len - 22).rev() {
        if cancellation.is_cancelled() {
            return Err(ArchiveCodecError::cancelled());
        }
        if window[offset..offset + 4] == EOCD_SIGNATURE {
            return Ok(window_start + offset as u64);
        }
    }

    Err(ArchiveCodecError::invalid(
        "ZIP archive has no valid end record",
    ))
}

fn read_exact_at<R: Read + Seek>(
    reader: &mut R,
    offset: u64,
    length: usize,
) -> ArchiveCodecResult<Vec<u8>> {
    reader
        .seek(SeekFrom::Start(offset))
        .map_err(|_| ArchiveCodecError::io("failed to seek within archive"))?;
    let mut buffer = vec![0_u8; length];
    reader
        .read_exact(&mut buffer)
        .map_err(|_| ArchiveCodecError::io("archive region is truncated"))?;
    Ok(buffer)
}

fn u16_le(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn u32_le(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
