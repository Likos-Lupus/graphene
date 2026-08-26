use super::{ArchiveCodecError, ArchiveCodecResult};
use crate::CancellationToken;
use std::io::Read;

/// Maximum size of the sliding history window required by RFC 1951 back-references.
const WINDOW_SIZE: usize = 32 * 1024;
const REFILL_BUFFER_BYTES: usize = 8 * 1024;
const DRAIN_CHUNK_BYTES: usize = 64 * 1024;

/// Bounded in-memory RFC 1951 decoder for Stored/Deflate archive entries.
pub fn inflate_raw_bounded(
    input: &[u8],
    expected: usize,
    max_output: usize,
    cancellation: &CancellationToken,
) -> ArchiveCodecResult<Vec<u8>> {
    if expected > max_output {
        return Err(ArchiveCodecError::invalid(
            "deflate output exceeds its configured limit",
        ));
    }

    let mut reader = RawDeflateReader::new(
        std::io::Cursor::new(input),
        max_output as u64,
        cancellation.clone(),
    );
    let mut output = Vec::new();
    let mut chunk = vec![0_u8; DRAIN_CHUNK_BYTES];
    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        output.extend_from_slice(&chunk[..read]);
    }

    Ok(output)
}

/// Streaming bounded RFC 1951 decoder pulling compressed input from any [`Read`] source.
///
/// Decoded bytes are produced through [`RawDeflateReader::read`] up to the configured
/// `max_output`; further production fails closed instead of buffering unbounded payload.
pub struct RawDeflateReader<R: Read> {
    bits: StreamBits<R>,
    max_output: u64,
    produced: u64,
    finished: bool,
    block: Option<BlockState>,
    window: Box<[u8]>,
    window_start: usize,
    window_len: usize,
    pending_copy: Option<PendingCopy>,
    cancellation: CancellationToken,
}

struct PendingCopy {
    distance: usize,
    remaining: usize,
}

enum BlockState {
    Stored {
        remaining: u32,
        final_block: bool,
    },
    Codes {
        literals: Huffman,
        distances: Huffman,
        final_block: bool,
    },
}

impl<R: Read> RawDeflateReader<R> {
    pub fn new(source: R, max_output: u64, cancellation: CancellationToken) -> Self {
        Self {
            bits: StreamBits::new(source),
            max_output,
            produced: 0,
            finished: false,
            block: None,
            window: vec![0_u8; WINDOW_SIZE].into_boxed_slice(),
            window_start: 0,
            window_len: 0,
            pending_copy: None,
            cancellation,
        }
    }

    /// Reads up to `buf.len()` decoded bytes, returning how many were written. Returns `Ok(0)`
    /// once the final block has been fully decoded.
    pub fn read(&mut self, buf: &mut [u8]) -> ArchiveCodecResult<usize> {
        if buf.is_empty() || self.finished {
            return Ok(0);
        }

        let mut written = 0_usize;
        while written < buf.len() {
            self.checkpoint()?;

            if let Some(copy) = self.pending_copy.take() {
                let budget = (self.max_output - self.produced) as usize;
                let count = copy.remaining.min(buf.len() - written).min(budget);
                if count == 0 {
                    return Err(ArchiveCodecError::invalid(
                        "deflate output exceeds its configured limit",
                    ));
                }
                // Ring position of the first source byte relative to current history state.
                let base = (self.window_start + self.window_len + WINDOW_SIZE
                    - (copy.distance % WINDOW_SIZE))
                    % WINDOW_SIZE;
                for offset in 0..count {
                    let value = self.window[(base + offset) % WINDOW_SIZE];
                    buf[written + offset] = value;
                    self.push_history(value);
                }
                written += count;
                if count < copy.remaining {
                    self.pending_copy = Some(PendingCopy {
                        distance: copy.distance,
                        remaining: copy.remaining - count,
                    });
                }
                continue;
            }

            match self.block.take() {
                None => {
                    if self.finished {
                        break;
                    }
                    self.start_block()?;
                }
                Some(BlockState::Stored {
                    mut remaining,
                    final_block,
                }) => {
                    let budget = (self.max_output - self.produced) as usize;
                    let count = (remaining as usize).min(buf.len() - written).min(budget);
                    if count == 0 {
                        return Err(ArchiveCodecError::invalid(
                            "deflate output exceeds its configured limit",
                        ));
                    }
                    let filled = self.bits.read_bytes(&mut buf[written..written + count])?;
                    if filled == 0 && count > 0 {
                        return Err(ArchiveCodecError::invalid(
                            "deflate stored block ended unexpectedly",
                        ));
                    }
                    for value in &buf[written..written + filled] {
                        self.push_history(*value);
                    }
                    written += filled;
                    remaining -= filled as u32;
                    if remaining == 0 {
                        self.bits.align_byte();
                        if final_block {
                            self.finished = true;
                            break;
                        }
                    } else {
                        self.block = Some(BlockState::Stored {
                            remaining,
                            final_block,
                        });
                    }
                }
                Some(BlockState::Codes {
                    literals,
                    distances,
                    final_block,
                }) => match literals.decode(&mut self.bits)? {
                    symbol @ 0..=255 => {
                        if self.produced >= self.max_output {
                            return Err(ArchiveCodecError::invalid(
                                "deflate output exceeds its configured limit",
                            ));
                        }
                        let value = symbol as u8;
                        buf[written] = value;
                        self.push_history(value);
                        written += 1;
                        self.block = Some(BlockState::Codes {
                            literals,
                            distances,
                            final_block,
                        });
                    }
                    256 => {
                        if final_block {
                            self.finished = true;
                            break;
                        }
                    }
                    symbol @ 257..=285 => {
                        let index = usize::from(symbol - 257);
                        let length =
                            LENGTH_BASE[index] + self.bits.read_bits(LENGTH_EXTRA[index])? as usize;
                        let distance_symbol = distances.decode(&mut self.bits)? as usize;
                        if distance_symbol >= DIST_BASE.len() {
                            return Err(ArchiveCodecError::invalid(
                                "deflate distance symbol is invalid",
                            ));
                        }
                        let distance = DIST_BASE[distance_symbol]
                            + self.bits.read_bits(DIST_EXTRA[distance_symbol])? as usize;
                        if distance == 0 || distance > self.window_len {
                            return Err(ArchiveCodecError::invalid(
                                "deflate back-reference distance is invalid",
                            ));
                        }
                        self.pending_copy = Some(PendingCopy {
                            distance,
                            remaining: length,
                        });
                        self.block = Some(BlockState::Codes {
                            literals,
                            distances,
                            final_block,
                        });
                    }
                    _ => {
                        return Err(ArchiveCodecError::invalid(
                            "deflate literal/length symbol is invalid",
                        ));
                    }
                },
            }
        }

        Ok(written)
    }

    /// Returns whether the compressed stream has been fully decoded.
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.finished
    }

    fn start_block(&mut self) -> ArchiveCodecResult<()> {
        let final_block = self.bits.read_bits(1)? != 0;
        let block_type = self.bits.read_bits(2)?;

        match block_type {
            0 => {
                self.bits.align_byte();
                let len = self.bits.read_bits(16)? as u16;
                let nlen = self.bits.read_bits(16)? as u16;
                if len != !nlen {
                    return Err(ArchiveCodecError::invalid(
                        "deflate stored block length check failed",
                    ));
                }
                self.block = Some(BlockState::Stored {
                    remaining: u32::from(len),
                    final_block,
                });
            }
            1 => {
                let (literals, distances) = fixed_trees()?;
                self.block = Some(BlockState::Codes {
                    literals,
                    distances,
                    final_block,
                });
            }
            2 => {
                let (literals, distances) = dynamic_trees(&mut self.bits)?;
                self.block = Some(BlockState::Codes {
                    literals,
                    distances,
                    final_block,
                });
            }
            _ => {
                return Err(ArchiveCodecError::invalid(
                    "deflate stream contains a reserved block type",
                ));
            }
        }

        Ok(())
    }

    fn push_history(&mut self, value: u8) {
        self.produced += 1;
        let index = (self.window_start + self.window_len) % WINDOW_SIZE;
        self.window[index] = value;
        if self.window_len < WINDOW_SIZE {
            self.window_len += 1;
        } else {
            self.window_start = (self.window_start + 1) % WINDOW_SIZE;
        }
    }

    fn checkpoint(&mut self) -> ArchiveCodecResult<()> {
        if self.cancellation.is_cancelled() {
            Err(ArchiveCodecError::cancelled())
        } else {
            Ok(())
        }
    }
}

impl<R: Read> Read for RawDeflateReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        RawDeflateReader::read(self, buf).map_err(|error| {
            let kind = if error.is_cancelled() {
                // Cooperative cancellation surfaces as Interrupted so callers can recover the
                // original semantic after wrapping this reader in std IO plumbing.
                std::io::ErrorKind::Interrupted
            } else {
                std::io::ErrorKind::Other
            };
            std::io::Error::new(kind, error.to_string())
        })
    }
}

struct StreamBits<R: Read> {
    source: R,
    buffer: Box<[u8]>,
    cursor: usize,
    end: usize,
    bit: usize,
    eof: bool,
}

impl<R: Read> StreamBits<R> {
    fn new(source: R) -> Self {
        Self {
            source,
            buffer: vec![0_u8; REFILL_BUFFER_BYTES].into_boxed_slice(),
            cursor: 0,
            end: 0,
            bit: 0,
            eof: false,
        }
    }

    fn refill(&mut self) -> ArchiveCodecResult<()> {
        if self.eof {
            return Err(ArchiveCodecError::invalid("deflate bitstream is truncated"));
        }
        if self.cursor < self.end {
            self.buffer.copy_within(self.cursor..self.end, 0);
            self.end -= self.cursor;
            self.cursor = 0;
        } else {
            self.cursor = 0;
            self.end = 0;
        }

        while self.end < self.buffer.len() {
            let read = self
                .source
                .read(&mut self.buffer[self.end..])
                .map_err(|_| ArchiveCodecError::io("failed to read compressed archive data"))?;
            if read == 0 {
                self.eof = true;
                break;
            }
            self.end += read;
        }

        if self.end == self.cursor {
            return Err(ArchiveCodecError::invalid("deflate bitstream is truncated"));
        }
        Ok(())
    }

    fn read_bits(&mut self, count: usize) -> ArchiveCodecResult<u32> {
        if count > 24 {
            return Err(ArchiveCodecError::invalid("deflate bitstream is truncated"));
        }

        let mut value = 0_u32;
        for shift in 0..count {
            if self.cursor >= self.end {
                self.refill()?;
            }
            let byte = self.buffer[self.cursor];
            let bit = (byte >> self.bit) & 1;
            value |= u32::from(bit) << shift;
            self.bit += 1;
            if self.bit == 8 {
                self.bit = 0;
                self.cursor += 1;
            }
        }

        Ok(value)
    }

    fn read_bytes(&mut self, out: &mut [u8]) -> ArchiveCodecResult<usize> {
        if self.bit != 0 {
            return Err(ArchiveCodecError::invalid(
                "deflate stored block requires byte alignment",
            ));
        }

        let mut written = 0_usize;
        while written < out.len() {
            if self.cursor >= self.end {
                self.refill()?;
            }
            let available = self.end - self.cursor;
            let count = available.min(out.len() - written);
            out[written..written + count]
                .copy_from_slice(&self.buffer[self.cursor..self.cursor + count]);
            self.cursor += count;
            written += count;
        }

        Ok(written)
    }

    fn align_byte(&mut self) {
        if self.bit != 0 {
            self.bit = 0;
            self.cursor += 1;
        }
    }
}

#[derive(Clone)]
struct Huffman {
    by_length: Vec<Vec<(u32, u16)>>,
    max_length: usize,
}

impl Huffman {
    fn from_lengths(lengths: &[u8]) -> ArchiveCodecResult<Self> {
        let max_length = lengths.iter().copied().max().unwrap_or(0) as usize;
        if max_length == 0 || max_length > 15 {
            return Err(ArchiveCodecError::invalid(
                "deflate Huffman tree has invalid code lengths",
            ));
        }

        let mut counts = vec![0_u32; max_length + 1];
        for &length in lengths {
            if length as usize > max_length {
                return Err(ArchiveCodecError::invalid(
                    "deflate Huffman code length is invalid",
                ));
            }
            if length != 0 {
                counts[length as usize] += 1;
            }
        }

        let mut code = 0_u32;
        let mut next = vec![0_u32; max_length + 1];
        for bits in 1..=max_length {
            code = (code + counts[bits - 1]) << 1;
            next[bits] = code;
        }

        if code + counts[max_length] > (1_u32 << max_length) {
            return Err(ArchiveCodecError::invalid(
                "deflate Huffman tree is oversubscribed",
            ));
        }

        let mut by_length = vec![Vec::new(); max_length + 1];
        for (symbol, &length) in lengths.iter().enumerate() {
            if length == 0 {
                continue;
            }
            let index = length as usize;
            let canonical = next[index];
            next[index] += 1;
            // DEFLATE transmits Huffman codes most-significant canonical bit first relative to
            // the bit-packed stream's LSB convention. Reverse to match read_bits accumulation.
            by_length[index].push((reverse_bits(canonical, index), symbol as u16));
        }

        Ok(Self {
            by_length,
            max_length,
        })
    }

    fn decode<B: Bits>(&self, bits: &mut B) -> ArchiveCodecResult<u16> {
        let mut code = 0_u32;
        for length in 1..=self.max_length {
            code |= bits.read_bits(1)? << (length - 1);
            if let Some((_, symbol)) = self.by_length[length]
                .iter()
                .find(|(candidate, _)| *candidate == code)
            {
                return Ok(*symbol);
            }
        }

        Err(ArchiveCodecError::invalid(
            "deflate Huffman symbol is invalid",
        ))
    }
}

trait Bits {
    fn read_bits(&mut self, count: usize) -> ArchiveCodecResult<u32>;
}

impl<R: Read> Bits for StreamBits<R> {
    fn read_bits(&mut self, count: usize) -> ArchiveCodecResult<u32> {
        StreamBits::read_bits(self, count)
    }
}

fn reverse_bits(mut value: u32, length: usize) -> u32 {
    let mut out = 0_u32;
    for _ in 0..length {
        out = (out << 1) | (value & 1);
        value >>= 1;
    }
    out
}

fn fixed_trees() -> ArchiveCodecResult<(Huffman, Huffman)> {
    let mut literals = vec![0_u8; 288];
    literals[0..=143].fill(8);
    literals[144..=255].fill(9);
    literals[256..=279].fill(7);
    literals[280..=287].fill(8);
    let distances = vec![5_u8; 32];

    Ok((
        Huffman::from_lengths(&literals)?,
        Huffman::from_lengths(&distances)?,
    ))
}

fn dynamic_trees<B: Bits>(bits: &mut B) -> ArchiveCodecResult<(Huffman, Huffman)> {
    let literal_count = bits.read_bits(5)? as usize + 257;
    let distance_count = bits.read_bits(5)? as usize + 1;
    let code_count = bits.read_bits(4)? as usize + 4;

    if literal_count > 286 || distance_count > 32 {
        return Err(ArchiveCodecError::invalid(
            "deflate dynamic tree has invalid symbol counts",
        ));
    }

    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let mut code_lengths = vec![0_u8; 19];
    for &symbol in &ORDER[..code_count] {
        code_lengths[symbol] = bits.read_bits(3)? as u8;
    }

    let code_tree = Huffman::from_lengths(&code_lengths)?;
    let total = literal_count + distance_count;
    let mut lengths = Vec::with_capacity(total);

    while lengths.len() < total {
        let symbol = code_tree.decode(bits)?;
        match symbol {
            0..=15 => lengths.push(symbol as u8),
            16 => {
                let previous = *lengths.last().ok_or_else(|| {
                    ArchiveCodecError::invalid("deflate repeat code has no previous length")
                })?;
                let repeat = bits.read_bits(2)? as usize + 3;
                if lengths
                    .len()
                    .checked_add(repeat)
                    .is_none_or(|size| size > total)
                {
                    return Err(ArchiveCodecError::invalid(
                        "deflate repeated code lengths exceed the tree size",
                    ));
                }
                lengths.extend(std::iter::repeat_n(previous, repeat));
            }
            17 => {
                let repeat = bits.read_bits(3)? as usize + 3;
                if lengths
                    .len()
                    .checked_add(repeat)
                    .is_none_or(|size| size > total)
                {
                    return Err(ArchiveCodecError::invalid(
                        "deflate zero code lengths exceed the tree size",
                    ));
                }
                lengths.extend(std::iter::repeat_n(0_u8, repeat));
            }
            18 => {
                let repeat = bits.read_bits(7)? as usize + 11;
                if lengths
                    .len()
                    .checked_add(repeat)
                    .is_none_or(|size| size > total)
                {
                    return Err(ArchiveCodecError::invalid(
                        "deflate long zero code lengths exceed the tree size",
                    ));
                }
                lengths.extend(std::iter::repeat_n(0_u8, repeat));
            }
            _ => {
                return Err(ArchiveCodecError::invalid(
                    "deflate code-length symbol is invalid",
                ));
            }
        }
    }

    if lengths.get(256).copied().unwrap_or(0) == 0 {
        return Err(ArchiveCodecError::invalid(
            "deflate literal tree has no end-of-block symbol",
        ));
    }

    Ok((
        Huffman::from_lengths(&lengths[..literal_count])?,
        Huffman::from_lengths(&lengths[literal_count..])?,
    ))
}

const LENGTH_BASE: [usize; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [usize; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [usize; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [usize; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
