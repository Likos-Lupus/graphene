use super::{ArchiveCodecError, ArchiveCodecResult, file_reader::crc32_incremental};
use std::io::{Seek, SeekFrom, Write};

/// Fixed DOS timestamp (1980-01-01 00:00:00) so archives never leak host modification times.
const FIXED_DOS_TIME: u16 = 0;
const FIXED_DOS_DATE: u16 = 0x0021;

/// Deterministic stored-method ZIP writer.
///
/// Entries are written uncompressed with normalized metadata and fixed timestamps so the same
/// entry sequence produces byte-identical output on every host. Entry payloads may be streamed
/// through [`DeterministicZipWriter::start_entry`].
pub struct DeterministicZipWriter<W: Write + Seek> {
    inner: W,
    offset: u64,
    records: Vec<CentralRecord>,
    finished: bool,
}

struct CentralRecord {
    name: String,
    crc32: u32,
    size: u64,
    local_offset: u64,
}

/// Streaming handle for one open archive entry.
pub struct ZipWriterEntry<'a, W: Write + Seek> {
    writer: &'a mut DeterministicZipWriter<W>,
    name: String,
    local_offset: u64,
    data_offset: u64,
    crc: u32,
    written: u64,
    sealed: bool,
}

impl<W: Write + Seek> DeterministicZipWriter<W> {
    /// Creates a writer over any seekable sink.
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            offset: 0,
            records: Vec::new(),
            finished: false,
        }
    }

    /// Returns the underlying sink. The archive must have been finalized first.
    pub fn into_inner(self) -> ArchiveCodecResult<W> {
        if !self.finished {
            return Err(ArchiveCodecError::invalid(
                "cannot return an unfinalized archive",
            ));
        }
        Ok(self.inner)
    }

    /// Adds one complete entry from an in-memory payload.
    pub fn add_entry(&mut self, name: &str, contents: &[u8]) -> ArchiveCodecResult<()> {
        let mut entry = self.start_entry(name)?;
        entry
            .write_all(contents)
            .map_err(|_| ArchiveCodecError::io("failed to write archive entry payload"))?;
        entry.finish()
    }

    /// Opens a streaming entry; call [`ZipWriterEntry::finish`] exactly once to seal it.
    pub fn start_entry(&mut self, name: &str) -> ArchiveCodecResult<ZipWriterEntry<'_, W>> {
        if self.finished {
            return Err(ArchiveCodecError::invalid("archive is already finalized"));
        }
        if name.is_empty() || name.len() > 1024 || name.contains('\\') || name.contains('\0') {
            return Err(ArchiveCodecError::invalid("archive entry name is invalid"));
        }
        if self.records.iter().any(|record| record.name == name) {
            return Err(ArchiveCodecError::invalid("duplicate archive entry name"));
        }

        let local_offset = self.offset;
        let name_bytes = name.as_bytes();
        let header_len = 30 + name_bytes.len() as u64;
        let data_offset = local_offset + header_len;

        let mut header = Vec::with_capacity(30 + name_bytes.len());
        header.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
        header.extend_from_slice(&20_u16.to_le_bytes()); // version needed
        header.extend_from_slice(&0_u16.to_le_bytes()); // flags
        header.extend_from_slice(&0_u16.to_le_bytes()); // stored
        header.extend_from_slice(&FIXED_DOS_TIME.to_le_bytes());
        header.extend_from_slice(&FIXED_DOS_DATE.to_le_bytes());
        header.extend_from_slice(&0_u32.to_le_bytes()); // crc placeholder
        header.extend_from_slice(&0_u32.to_le_bytes()); // compressed placeholder
        header.extend_from_slice(&0_u32.to_le_bytes()); // uncompressed placeholder
        header.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        header.extend_from_slice(&0_u16.to_le_bytes()); // extra length
        header.extend_from_slice(name_bytes);

        self.inner
            .write_all(&header)
            .map_err(|_| ArchiveCodecError::io("failed to write archive local header"))?;
        self.offset = data_offset;

        Ok(ZipWriterEntry {
            writer: self,
            name: name.to_owned(),
            local_offset,
            data_offset,
            crc: 0xffff_ffff_u32,
            written: 0,
            sealed: false,
        })
    }

    /// Seals the archive by writing the central directory and end record.
    pub fn finish(&mut self) -> ArchiveCodecResult<u64> {
        if self.finished {
            return Err(ArchiveCodecError::invalid("archive is already finalized"));
        }
        self.finished = true;

        let central_offset = self.offset;
        for record in &self.records {
            let mut entry = Vec::with_capacity(46 + record.name.len());
            entry.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
            entry.extend_from_slice(&20_u16.to_le_bytes()); // version made by (host 0)
            entry.extend_from_slice(&20_u16.to_le_bytes()); // version needed
            entry.extend_from_slice(&0_u16.to_le_bytes()); // flags
            entry.extend_from_slice(&0_u16.to_le_bytes()); // stored
            entry.extend_from_slice(&FIXED_DOS_TIME.to_le_bytes());
            entry.extend_from_slice(&FIXED_DOS_DATE.to_le_bytes());
            entry.extend_from_slice(&record.crc32.to_le_bytes());
            entry.extend_from_slice(&(record.size as u32).to_le_bytes());
            entry.extend_from_slice(&(record.size as u32).to_le_bytes());
            entry.extend_from_slice(&(record.name.len() as u16).to_le_bytes());
            entry.extend_from_slice(&0_u16.to_le_bytes()); // extra length
            entry.extend_from_slice(&0_u16.to_le_bytes()); // comment length
            entry.extend_from_slice(&0_u16.to_le_bytes()); // disk number
            entry.extend_from_slice(&0_u16.to_le_bytes()); // internal attrs
            entry.extend_from_slice(&0_u32.to_le_bytes()); // external attrs
            entry.extend_from_slice(&(record.local_offset as u32).to_le_bytes());
            entry.extend_from_slice(record.name.as_bytes());

            self.inner
                .write_all(&entry)
                .map_err(|_| ArchiveCodecError::io("failed to write archive central directory"))?;
        }
        let end_position = self
            .inner
            .stream_position()
            .map_err(|_| ArchiveCodecError::io("failed to measure archive"))?;
        let central_size = end_position.saturating_sub(central_offset);

        let mut eocd = Vec::with_capacity(22);
        eocd.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        eocd.extend_from_slice(&0_u16.to_le_bytes());
        eocd.extend_from_slice(&0_u16.to_le_bytes());
        eocd.extend_from_slice(&(self.records.len() as u16).to_le_bytes());
        eocd.extend_from_slice(&(self.records.len() as u16).to_le_bytes());
        eocd.extend_from_slice(&(central_size as u32).to_le_bytes());
        eocd.extend_from_slice(&(central_offset as u32).to_le_bytes());
        eocd.extend_from_slice(&0_u16.to_le_bytes()); // comment length
        self.inner
            .write_all(&eocd)
            .map_err(|_| ArchiveCodecError::io("failed to write archive end record"))?;

        Ok(self.offset)
    }

    fn patch_entry_header(
        &mut self,
        local_offset: u64,
        crc: u32,
        size: u64,
    ) -> ArchiveCodecResult<()> {
        let previous = self
            .inner
            .stream_position()
            .map_err(|_| ArchiveCodecError::io("failed to measure archive writer position"))?;
        self.inner
            .seek(SeekFrom::Start(local_offset + 14))
            .map_err(|_| ArchiveCodecError::io("failed to seek for archive header patch"))?;
        let mut fields = Vec::with_capacity(12);
        fields.extend_from_slice(&crc.to_le_bytes());
        fields.extend_from_slice(&(size as u32).to_le_bytes());
        fields.extend_from_slice(&(size as u32).to_le_bytes());
        self.inner
            .write_all(&fields)
            .map_err(|_| ArchiveCodecError::io("failed to patch archive local header"))?;
        self.inner
            .seek(SeekFrom::Start(previous))
            .map_err(|_| ArchiveCodecError::io("failed to restore archive writer position"))?;
        Ok(())
    }
}

impl<W: Write + Seek> Write for ZipWriterEntry<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.inner.write_all(buf)?;
        self.crc = crc32_incremental(self.crc, buf);
        self.written += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.inner.flush()
    }
}

impl<W: Write + Seek> Drop for ZipWriterEntry<'_, W> {
    fn drop(&mut self) {
        // Best-effort sealing so an early `?` still leaves a structurally valid archive; callers
        // that care about explicit completion use `finish()` directly.
        let _ = self.seal();
    }
}

impl<W: Write + Seek> ZipWriterEntry<'_, W> {
    /// Seals this entry, patches its header, and registers it in the central directory.
    pub fn finish(mut self) -> ArchiveCodecResult<()> {
        self.seal()?;
        Ok(())
    }

    fn seal(&mut self) -> ArchiveCodecResult<u32> {
        if self.sealed {
            return Ok(self.crc ^ 0xffff_ffff_u32);
        }
        self.sealed = true;
        let crc = self.crc ^ 0xffff_ffff_u32;
        let name = self.name.clone();
        let local_offset = self.local_offset;
        let data_offset = self.data_offset;
        let written = self.written;
        let writer = &mut *self.writer;

        writer
            .inner
            .flush()
            .map_err(|_| ArchiveCodecError::io("failed to flush archive entry"))?;
        writer.patch_entry_header(local_offset, crc, written)?;
        writer.offset = data_offset + written;
        writer.records.push(CentralRecord {
            name,
            crc32: crc,
            size: written,
            local_offset,
        });
        Ok(crc)
    }
}
