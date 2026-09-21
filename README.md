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

## Reference hosts

`apps/` is a separate Cargo workspace containing two reference consumers of the root `graphene`
facade, plus shared host-only support:

- `apps/graphene-cli` — reference CLI covering every major service family with human and stable
  `--json` output modes.
- `apps/graphene-tauri` — Vite/TypeScript frontend plus a `src-tauri` Rust adapter that consumes
  Graphene over Tauri IPC and typed events.
- `apps/graphene-reference-host-support` — host-only data-root/config resolution, a production OS
  credential-vault `SecretStore` adapter, tracing initialization, and operation/run bridging
  registries.

Hosts are outward consumers: they never contain Minecraft/provider/install/content/modpack/
diagnostic/launch logic, and no host framework type enters the engine. See
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and [`docs/SECURITY.md`](docs/SECURITY.md) for the
host dependency direction and trust boundaries.
