//! Provider-neutral networking, retry, streaming, and integrity verification for Graphene.
//!
//! Concrete `reqwest` types stay private. The service layer supplies Graphene-managed temporary
//! paths and performs storage commits after this crate has verified the transfer.

mod client;
mod config;
mod retry;
mod verify;

pub use client::{BoundedResponse, NetworkClient, TransferResult, VerifiedFile};
pub use config::{NetworkConfig, ProxyPolicy, RedirectPolicy};
pub use retry::RetryPolicy;
pub use verify::verify_transfer;
