# ADR-0001: Establish Graphene as a UI-Independent Launcher Engine

- **Status:** Accepted
- **Date:** 2026-08-07

## Context

The repository begins as an empty Rust 2024 library. The long-term goal is to support the non-UI functionality of a full Minecraft: Java Edition launcher and allow future front ends such as Tauri, Slint, or CLI applications.

If implementation begins inside a single crate with direct provider calls, UI callbacks, and loader-specific branches, later separation will be expensive.

## Decision

1. Keep `graphene` as the root facade crate.
2. Convert the repository to a Cargo workspace.
3. Organize implementation into bounded-context crates.
4. Keep all UI frameworks outside the backend.
5. Normalize external provider data at adapter boundaries.
6. Make `Artifact`, component resolution, `InstallPlan`, and `LaunchPlan` central abstractions.
7. Use transaction-based instance mutation.
8. Use capability-based provider extension.
9. Treat files as authoritative user-owned state and databases as disposable indexes/caches.
10. Delay runtime plugin ABI design until a real requirement exists.

## Consequences

### Positive
- Tauri, Slint, and CLI can share the same engine;
- provider changes are isolated;
- install and launch behavior can be tested deterministically;
- repair can reuse installation infrastructure;
- loaders and content providers can expand without rewriting orchestration.

### Negative
- more initial type/interface design;
- more crates than a prototype;
- some features require normalization work instead of exposing provider responses directly;
- strict dependency rules must be maintained actively.

## Revisit Conditions

Revisit this ADR only if:
- the crate graph creates proven operational cost that outweighs its isolation benefits;
- a host-specific requirement cannot be represented through typed requests/events without materially weakening usability;
- plugin requirements justify a versioned IPC/WASM extension protocol.
