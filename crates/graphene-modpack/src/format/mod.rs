//! Content-based pack format detection and per-format adapters.

pub mod curseforge;
pub mod generic;
pub mod graphene;
pub mod modrinth;
pub mod multimc;

mod detect;

pub use detect::{DetectedFormatRoots, FormatDetection, detect_pack_format};
