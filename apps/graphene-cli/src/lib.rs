//! Reference Graphene CLI.
//!
//! The CLI is an outward consumer of the root `graphene` facade. Command handlers perform input
//! validation, presentation mapping, operation-handle ownership, and process lifecycle glue only;
//! Minecraft, provider, install, content, modpack, diagnostic, and launch logic stay in the engine.

mod app;
mod cli;
mod commands;
mod context;
mod error;
mod interaction;
mod output;

pub use app::main_entry;
