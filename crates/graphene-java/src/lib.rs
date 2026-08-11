//! Local-only Java discovery, bounded probing, compatibility evaluation, and deterministic selection.
//!
//! The crate root is a public facade; discovery, probing, normalization, and selection each own
//! their internal implementation details.

mod diagnostic;
mod discovery;
mod error;
mod managed;
mod model;
mod probe;
mod selection;
mod version;

pub use diagnostic::compatibility_diagnostic;
pub use discovery::{MAX_JAVA_CANDIDATES, discover_java_candidates};
pub use managed::{
    JavaDistributionCapabilities, JavaDistributionFuture, JavaDistributionProvider, JavaImageKind,
    JavaOperatingSystem, MANAGED_JAVA_PLAN_VERSION, MANAGED_RUNTIME_SCHEMA_VERSION,
    ManagedArchiveFormat, ManagedJavaInstallPlan, ManagedJavaRelease, ManagedJavaRequest,
    ManagedJavaRuntime, select_managed_runtime,
};
pub use model::{
    JavaArchitecture, JavaCandidate, JavaCandidateSource, JavaRequirement, JavaRuntime, JavaVendor,
};
pub use probe::{JAVA_PROBE_OUTPUT_LIMIT, JAVA_PROBE_TIMEOUT, probe_java};
pub use selection::{is_compatible, select_java};
pub use version::{normalize_vendor, parse_java_major};
