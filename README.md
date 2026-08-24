# Graphene

Graphene is a **UI-independent Minecraft: Java Edition launcher engine** written in Rust. Tauri,
Slint, CLI, and other hosts are consumers of the engine rather than part of its domain.

The current repository implements the launcher foundation, Vanilla metadata/install/launch path,
provider-neutral accounts with Microsoft authentication support, managed Java support, component
composition, Fabric/Forge/NeoForge loader adapters, mutable instance engine with locking, verify,
and repair, and provider-neutral content management (local offline mod scanning, Modrinth and
CurseForge catalog integration, bounded dependency resolution, journaled transactions, and lockfile
convergence). The implementation remains a candidate rather than release sign-off; current automated
and real-world validation evidence is tracked in [`docs/VALIDATION.md`](docs/VALIDATION.md).

## Start here

- [`docs/PROJECT_SPECIFICATION.md`](docs/PROJECT_SPECIFICATION.md) — product, scope, final target,
  and non-goals.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — dependency direction and bounded contexts.
- [`docs/API.md`](docs/API.md) — current public behavior.
- [`docs/SECURITY.md`](docs/SECURITY.md) — trust boundaries and residual risks.
- [`docs/VALIDATION.md`](docs/VALIDATION.md) — automated and real-world validation status.
- [`docs/LOADER_SUPPORT.md`](docs/LOADER_SUPPORT.md) — loader compatibility evidence.
- [`docs/ROADMAP.md`](docs/ROADMAP.md) — implementation chronology and future work.
- [`docs/ENGINEERING_STANDARDS.md`](docs/ENGINEERING_STANDARDS.md) — testing, modularity,
  documentation, and ADR policy.

## Architecture rule of thumb

> Stable Graphene-owned domain models point inward; volatile providers and host frameworks stay at
> the edges.

The root `graphene` crate is the consumer-facing facade. Provider DTOs, Reqwest responses, archive
codec internals, filesystem publication primitives, and Tokio child handles are implementation
details behind bounded-context crates.
