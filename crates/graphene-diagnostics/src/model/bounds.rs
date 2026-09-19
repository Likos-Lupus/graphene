//! Named hard ceilings for diagnostic collection, analysis, and report retention.
//!
//! Public configuration may lower these limits but can never raise them.

/// Maximum findings retained in one diagnostic report.
pub const MAX_FINDINGS_PER_REPORT: usize = 512;
/// Maximum evidence records retained in one diagnostic report.
pub const MAX_EVIDENCE_PER_REPORT: usize = 512;
/// Maximum recommendations retained in one diagnostic report.
pub const MAX_RECOMMENDATIONS_PER_REPORT: usize = 512;
/// Maximum source summaries retained in one diagnostic report.
pub const MAX_SOURCES_PER_REPORT: usize = 64;
/// Maximum stored excerpt bytes per evidence record after redaction.
pub const MAX_EXCERPT_BYTES: usize = 4 * 1024;
/// Maximum bytes inspected from a single textual source.
pub const MAX_SOURCE_BYTES: u64 = 8 * 1024 * 1024;
/// Maximum aggregate textual bytes inspected across all sources.
pub const MAX_AGGREGATE_BYTES: u64 = 32 * 1024 * 1024;
/// Maximum crash reports selected from one instance.
pub const MAX_CRASH_REPORTS: usize = 8;
/// Maximum JVM fatal-error logs selected from one instance.
pub const MAX_HS_ERR_REPORTS: usize = 8;
/// Maximum single line length retained before truncation.
pub const MAX_LINE_BYTES: usize = 64 * 1024;
/// Schema version stamped on every diagnostic report.
pub const DIAGNOSTIC_REPORT_SCHEMA_VERSION: u32 = 1;
