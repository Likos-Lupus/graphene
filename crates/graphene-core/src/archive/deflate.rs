use super::{ArchiveCodecError, ArchiveCodecResult};
use crate::CancellationToken;

// Minimal bounded RFC 1951 decoder for Stored/Deflate archive entries.
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

    let mut bits = BitReader::new(input);
    let mut output = Vec::with_capacity(expected.min(1024 * 1024));

    loop {
        checkpoint(cancellation)?;
        let final_block = bits.read_bits(1)? != 0;
        let block_type = bits.read_bits(2)?;

        match block_type {
            0 => inflate_stored(&mut bits, &mut output, expected, max_output, cancellation)?,
            1 => {
                let (literal, distance) = fixed_trees()?;
                inflate_codes(
                    &mut bits,
                    &mut output,
                    expected,
                    max_output,
                    &literal,
                    &distance,
                    cancellation,
                )?;
            }
            2 => {
                let (literal, distance) = dynamic_trees(&mut bits)?;
                inflate_codes(
                    &mut bits,
                    &mut output,
                    expected,
                    max_output,
                    &literal,
                    &distance,
                    cancellation,
                )?;
            }
            _ => {
                return Err(ArchiveCodecError::invalid(
                    "deflate stream contains a reserved block type",
                ));
            }
        }

        if final_block {
            break;
        }
    }

    Ok(output)
}

struct BitReader<'a> {
    bytes: &'a [u8],
    bit: usize,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, bit: 0 }
    }

    fn read_bits(&mut self, count: usize) -> ArchiveCodecResult<u32> {
        if count > 24
            || self
                .bit
                .checked_add(count)
                .is_none_or(|end| end > self.bytes.len() * 8)
        {
            return Err(ArchiveCodecError::invalid("deflate bitstream is truncated"));
        }

        let mut value = 0u32;
        for shift in 0..count {
            let byte = self.bytes[self.bit / 8];
            let bit = (byte >> (self.bit % 8)) & 1;
            value |= u32::from(bit) << shift;
            self.bit += 1;
        }

        Ok(value)
    }

    fn align_byte(&mut self) {
        self.bit = (self.bit + 7) & !7;
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

        let mut counts = vec![0u32; max_length + 1];
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

        let mut code = 0u32;
        let mut next = vec![0u32; max_length + 1];
        for bits in 1..=max_length {
            code = (code + counts[bits - 1]) << 1;
            next[bits] = code;
        }

        if code + counts[max_length] > (1u32 << max_length) {
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
            // DEFLATE transmits Huffman codes most-significant canonical bit first relative to the
            // bit-packed stream's LSB convention. Reverse to match read_bits accumulation.
            by_length[index].push((reverse_bits(canonical, index), symbol as u16));
        }

        Ok(Self {
            by_length,
            max_length,
        })
    }

    fn decode(&self, bits: &mut BitReader<'_>) -> ArchiveCodecResult<u16> {
        let mut code = 0u32;
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

fn reverse_bits(mut value: u32, length: usize) -> u32 {
    let mut out = 0u32;
    for _ in 0..length {
        out = (out << 1) | (value & 1);
        value >>= 1;
    }
    out
}

fn fixed_trees() -> ArchiveCodecResult<(Huffman, Huffman)> {
    let mut literals = vec![0u8; 288];
    literals[0..=143].fill(8);
    literals[144..=255].fill(9);
    literals[256..=279].fill(7);
    literals[280..=287].fill(8);
    let distances = vec![5u8; 32];

    Ok((
        Huffman::from_lengths(&literals)?,
        Huffman::from_lengths(&distances)?,
    ))
}

fn dynamic_trees(bits: &mut BitReader<'_>) -> ArchiveCodecResult<(Huffman, Huffman)> {
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
    let mut code_lengths = vec![0u8; 19];
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
                lengths.extend(std::iter::repeat_n(0u8, repeat));
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
                lengths.extend(std::iter::repeat_n(0u8, repeat));
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

fn inflate_stored(
    bits: &mut BitReader<'_>,
    output: &mut Vec<u8>,
    expected: usize,
    max_output: usize,
    cancellation: &CancellationToken,
) -> ArchiveCodecResult<()> {
    bits.align_byte();
    let len = bits.read_bits(16)? as u16;
    let nlen = bits.read_bits(16)? as u16;

    if len != !nlen {
        return Err(ArchiveCodecError::invalid(
            "deflate stored block length check failed",
        ));
    }

    let count = usize::from(len);
    if output
        .len()
        .checked_add(count)
        .is_none_or(|size| size > expected || size > max_output)
    {
        return Err(ArchiveCodecError::invalid(
            "deflate stored block exceeds output bound",
        ));
    }

    for index in 0..count {
        if index % (64 * 1024) == 0 {
            checkpoint(cancellation)?;
        }
        output.push(bits.read_bits(8)? as u8);
    }

    Ok(())
}

fn inflate_codes(
    bits: &mut BitReader<'_>,
    output: &mut Vec<u8>,
    expected: usize,
    max_output: usize,
    literals: &Huffman,
    distances: &Huffman,
    cancellation: &CancellationToken,
) -> ArchiveCodecResult<()> {
    const LENGTH_BASE: [usize; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const LENGTH_EXTRA: [usize; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    const DIST_BASE: [usize; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    const DIST_EXTRA: [usize; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];
    loop {
        checkpoint(cancellation)?;
        match literals.decode(bits)? {
            symbol @ 0..=255 => {
                if output.len() >= expected || output.len() >= max_output {
                    return Err(ArchiveCodecError::invalid(
                        "deflate literal exceeds output bound",
                    ));
                }
                output.push(symbol as u8);
            }

            256 => return Ok(()),
            symbol @ 257..=285 => {
                let index = usize::from(symbol - 257);
                let length = LENGTH_BASE[index] + bits.read_bits(LENGTH_EXTRA[index])? as usize;
                let distance_symbol = distances.decode(bits)? as usize;

                if distance_symbol >= DIST_BASE.len() {
                    return Err(ArchiveCodecError::invalid(
                        "deflate distance symbol is invalid",
                    ));
                }

                let distance = DIST_BASE[distance_symbol]
                    + bits.read_bits(DIST_EXTRA[distance_symbol])? as usize;
                if distance == 0 || distance > output.len() {
                    return Err(ArchiveCodecError::invalid(
                        "deflate back-reference distance is invalid",
                    ));
                }

                if output
                    .len()
                    .checked_add(length)
                    .is_none_or(|size| size > expected || size > max_output)
                {
                    return Err(ArchiveCodecError::invalid(
                        "deflate back-reference exceeds output bound",
                    ));
                }

                for _ in 0..length {
                    let value = output[output.len() - distance];
                    output.push(value);
                }
            }
            _ => {
                return Err(ArchiveCodecError::invalid(
                    "deflate literal/length symbol is invalid",
                ));
            }
        }
    }
}
fn checkpoint(cancellation: &CancellationToken) -> ArchiveCodecResult<()> {
    if cancellation.is_cancelled() {
        Err(ArchiveCodecError::cancelled())
    } else {
        Ok(())
    }
}
