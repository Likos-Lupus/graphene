//! Local-only Java discovery, bounded probing, compatibility evaluation, and deterministic selection.
//!
//! The crate root is a public facade; discovery, probing, normalization, and selection each own
//! their internal implementation details.

mod discovery;
mod error;
mod model;
mod probe;
mod selection;
mod version;

pub use discovery::{MAX_JAVA_CANDIDATES, discover_java_candidates};
pub use model::{
    JavaArchitecture, JavaCandidate, JavaCandidateSource, JavaRequirement, JavaRuntime, JavaVendor,
};
pub use probe::{JAVA_PROBE_OUTPUT_LIMIT, JAVA_PROBE_TIMEOUT, probe_java};
pub use selection::{is_compatible, select_java};
pub use version::{normalize_vendor, parse_java_major};
