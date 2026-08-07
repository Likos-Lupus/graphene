//! Phase 0 Graphene-managed storage root, layout, cache paths, and commit primitives.
//!
//! Filesystem paths are explicitly rooted per engine. There is no process-global data directory.

mod atomic;
mod cache;
mod layout;
mod root;
mod temp;

pub use cache::{CacheAddress, CacheAlgorithm};
pub use root::DataRoot;
