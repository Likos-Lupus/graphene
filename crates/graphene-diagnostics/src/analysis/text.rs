//! Bounded, lossy-safe text normalization and line/excerpt scanning primitives.

/// Decodes bytes with a documented lossy UTF-8 boundary instead of rejecting input.
#[must_use]
pub fn decode_lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Returns the longest char-boundary-safe prefix no longer than `max_bytes`.
#[must_use]
pub fn truncate_head(value: &str, max_bytes: usize) -> (&str, bool) {
    if value.len() <= max_bytes {
        return (value, false);
    }

    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }

    (&value[..end], true)
}

/// Returns the longest char-boundary-safe suffix no longer than `max_bytes`.
#[must_use]
pub fn truncate_tail(value: &str, max_bytes: usize) -> (&str, bool) {
    if value.len() <= max_bytes {
        return (value, false);
    }

    let mut start = value.len() - max_bytes;
    while start < value.len() && !value.is_char_boundary(start) {
        start += 1;
    }

    (&value[start..], true)
}

/// One normalized, length-bounded logical line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedLine {
    pub number: u64,
    pub text: String,
    pub truncated: bool,
}

/// Deterministic, allocation-bounded line scanner tolerating LF, CRLF, and truncated files.
pub struct LineScanner<'a> {
    text: &'a str,
    offset: usize,
    number: u64,
    max_line_bytes: usize,
}

impl<'a> LineScanner<'a> {
    /// Creates a line scanner over decoded text.
    #[must_use]
    pub fn new(text: &'a str, max_line_bytes: usize) -> Self {
        Self {
            text,
            offset: 0,
            number: 0,
            max_line_bytes,
        }
    }
}

impl Iterator for LineScanner<'_> {
    type Item = BoundedLine;

    fn next(&mut self) -> Option<BoundedLine> {
        if self.offset >= self.text.len() {
            return None;
        }

        let rest = &self.text[self.offset..];
        let (raw, consumed) = match rest.find('\n') {
            Some(position) => (&rest[..position], position + 1),
            None => (rest, rest.len()),
        };
        self.offset += consumed;

        let number = self.number;
        self.number += 1;

        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        let (text, truncated) = truncate_head(raw, self.max_line_bytes);
        Some(BoundedLine {
            number,
            text: text.to_owned(),
            truncated,
        })
    }
}

/// Convenience constructor for a bounded line scanner.
#[must_use]
pub fn bounded_lines(text: &str, max_line_bytes: usize) -> LineScanner<'_> {
    LineScanner::new(text, max_line_bytes)
}
