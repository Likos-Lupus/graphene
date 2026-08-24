# Graphene Implementation Roadmap

This is the canonical home for development chronology. Current behavior belongs in `API.md`, trust
boundaries in `SECURITY.md`, and validation status in `VALIDATION.md`.

## Phase 0 — Foundation

Workspace/facade, core operation/error/artifact types, platform/network/storage boundaries, verified
artifact acquisition, cancellation/progress, and deterministic local-fixture testing.

**Exit:** cross-platform build/test gates, verified download/cache behavior, safe cancellation, and
no UI dependency.

## Phase 1 — Vanilla end-to-end

Mojang metadata normalization, resolved Minecraft, deterministic create-only installation, local
Java selection, offline launch planning, and direct process lifecycle.

**Exit:** from an empty data root, install and launch a pinned Vanilla version through the library,
including restart/offline planning and immutable reuse evidence.

## Phase 2 — Authentication and managed Java

Provider-neutral accounts, Microsoft device/refresh chain, injected secure credential storage,
offline identities, managed Java resolution/staged installation, and Java diagnostics.

**Repository status:** implementation candidate. Real authentication/managed-Java smoke remains
`NOT RUN`, a production secure vault is host/distributor supplied, and Rust quality gates cannot run
in the current toolchain-less environment.

## Phase 3 — Loader components

Component graph/patch model, provider registry, Fabric/Forge/NeoForge adapters, Forge-family staged
Java preparation, generated-output verification/reuse, and receipt component persistence.

**Repository status:** implementation candidate. Deterministic fixture coverage exists; real loader
smokes and Rust quality gates remain pending. See `LOADER_SUPPORT.md` and `VALIDATION.md`.

## Phase 4 — Instance engine

Centralized committed-state repository, resilient inventory discovery, global defaults and
per-instance configuration hierarchy with explicit tri-state patch semantics, cross-process advisory
shared/exclusive leases (`fs2`) backed by persistent carriers, schema-versioned provider-neutral
desired-state lockfile, runtime lease retention with stale-plan protection, atomic rename, streamed
containment-safe clone, quarantine delete transactions, quick and full verification, and
deterministic repair planning and execution reusing install primitives.

**Exit:** repair is a deterministic planned transaction derived from durable desired state plus a
filesystem diff, mutable instance operations share one repository/lease/transaction model, and no
Phase 4 capability depends on ad hoc mutation or provider-specific persisted state.

## Phase 5 — Content

Provider-neutral content bounded context (`graphene-content`), offline mod scanning
(`.minecraft/mods`), bounded metadata inspection (Fabric `fabric.mod.json`, Forge
`META-INF/mods.toml`, NeoForge `META-INF/neoforge.mods.toml`, Legacy `mcmod.info`), streaming SHA-1
and SHA-256 local identities, CurseForge-compatible Murmur2 lookup fingerprinting, Modrinth and
optional CurseForge providers with strict secret redaction, deterministic bounded
required-dependency resolution, inspectable non-mutating `ContentMutationPlan`, journaled
crash-recoverable execution under exclusive instance leases, schema-2 `lock.json` desired-state
evolution with schema-1 read compatibility, and convergence with existing instance verification and
provider-neutral repair.

**Exit:** hosts have provider-neutral local mod inventory, discovery, recognition, exact
install/update and dependency/compatibility planning; all managed content mutations use one
inspectable stale-protected transaction model and persist into the existing desired-state lockfile;
verified artifact acquisition, instance verification, repair, clone/delete locking, cancellation,
security, and provider isolation remain convergent rather than forming a second launcher pipeline.

## Phase 6 — Modpacks

`.mrpack`, CurseForge packs, Prism/MultiMC import, generic local/URL archives, and a Graphene pack
format/export path normalized to a common install model with archive-security coverage.

## Phase 7 — Diagnostics

Verifier, Java diagnostics, crash/log collection, secret redaction, and structured repair
recommendations that hosts can present without string matching.

## Phase 8 — Host integration

Reference CLI plus at least one additional host adapter (for example Tauri or Slint), proving two
host types can use the same backend logic.

## Priority policy

P0 work preserves the install-to-launch backbone and safety boundaries. P1 completes a modern
launcher backend. P2 is ecosystem breadth/convenience and must not destabilize P0/P1 contracts.
