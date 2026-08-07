# Graphene

Graphene is a planned **UI-independent Minecraft: Java Edition launcher engine** written in Rust.

The current repository is intentionally at the architecture/foundation stage. The first
implementation target is a complete Vanilla install-to-launch vertical slice. Tauri, Slint, CLI, and
other hosts are consumers of the engine rather than part of its domain.

## Architecture Baseline

Start here:

- [`docs/PROJECT_SPECIFICATION.md`](docs/PROJECT_SPECIFICATION.md) — product definition, initial
  direction, final deliverable, architecture, boundaries, security, and Definition of Done.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — layer model, bounded contexts, and dependency
  rules.
- [`docs/SCOPE_AND_BOUNDARIES.md`](docs/SCOPE_AND_BOUNDARIES.md) — explicit ownership and non-goals.
- [`docs/ROADMAP.md`](docs/ROADMAP.md) — phased implementation order and exit criteria.
- [`docs/TARGET_DELIVERABLE.md`](docs/TARGET_DELIVERABLE.md) — what a complete Graphene backend must
  provide.
- [
  `docs/adr/0001-ui-independent-launcher-engine.md`](docs/adr/0001-ui-independent-launcher-engine.md) —
  initial architecture decision.

## First Engineering Target

```text
Minecraft metadata
      |
      v
ResolvedMinecraft
      |
      v
InstallPlan
      |
      v
Transactional install
      |
      v
Java selection
      |
      v
LaunchPlan
      |
      v
Minecraft process
```

The first major milestone is achieved when a clean Graphene data directory can install and launch a
Vanilla Minecraft instance without any UI framework dependency.

## Architectural Rule of Thumb

> Stable domain models point inward; volatile providers and host frameworks stay outside.

See the project specification before adding new crates, providers, loaders, or host integrations.
