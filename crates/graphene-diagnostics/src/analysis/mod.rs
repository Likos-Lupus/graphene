//! Bounded text normalization, line scanning, and deterministic secret/path redaction.

pub mod redaction;
pub mod text;

pub use redaction::{
    DATA_ROOT_PLACEHOLDER, HOME_PLACEHOLDER, RedactedExcerpt, RedactionContext, Redactor,
    SECRET_PLACEHOLDER,
};
pub use text::{
    BoundedLine, LineScanner, bounded_lines, decode_lossy, truncate_head, truncate_tail,
};
