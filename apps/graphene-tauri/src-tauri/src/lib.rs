//! Reference Tauri host for Graphene.
//!
//! The Rust side owns Graphene composition, host operation/run registries, wire DTO conversion, and
//! the Tauri event bridge. It consumes only the root `graphene` facade; no engine business logic is
//! duplicated here.

mod app;
mod commands;
mod dto;
mod error;
mod events;
mod state;

#[cfg(test)]
mod tests;

pub use app::run;
