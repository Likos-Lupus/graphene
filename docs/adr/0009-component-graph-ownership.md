# ADR-0009: Own Loader Component Graph and Patch Composition in graphene-minecraft

- **Status:** Accepted
- **Date:** 2026-08-12

## Context

Vanilla, Fabric, Forge, and NeoForge need to converge on one launch-facing Minecraft model. Putting
loader-family branches in `graphene-launch` or persisting synthetic Minecraft version IDs would make
repair/content work dependent on parsing provider conventions and would duplicate existing Minecraft
merge behavior.

## Decision

`graphene-minecraft` owns a provider-neutral bounded component graph and ordered
`MinecraftVersionPatch` composition.

- Every graph has exactly one `Minecraft` component and at most one primary `Loader` component.
- Primary loaders structurally conflict with the other primary loader UIDs.
- Requirements are intentionally limited to exact component/version dependencies and explicit
  presence; moving/range selection completes in provider adapters before graph construction.
- Cycles, missing requirements, conflicts, duplicates, and node/edge-limit violations fail before
  committed mutation.
- Topological ordering is deterministic and never depends on hash-map iteration.
- Vanilla is represented by `net.minecraft <exact-base>`; the base Minecraft version is persisted
  independently from loader identity.
- Provider patches compose into the existing launch-facing `ResolvedMinecraft` fields and reuse the
  existing Maven/rule semantics instead of adding a loader-specific library system.
- `graphene-launch` consumes only the committed normalized result and contains no loader-family
  branch.

## Consequences

A future loader normally adds a provider adapter that produces an exact component plus normalized
patch/preparation data. Future instance/content work can compare exact component sets and read
base/loader identity without reverse-engineering synthetic version strings.

The component solver is deliberately not a general mod/package dependency solver; arbitrary mods
remain outside this component graph.
