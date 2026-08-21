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

Instance repository, create/delete/rename/clone, global/per-instance configuration, locking,
lockfiles, verify, and repair.

**Exit:** repair is a planned transaction based on durable state/filesystem diff rather than ad hoc
mutation.

## Phase 5 — Content

Local mod scanning, normalized project/version/file/dependency models, provider adapters such as
Modrinth/CurseForge, hash lookup, install/update, and compatibility/dependency checks.

**Exit:** provider-neutral service APIs and artifact/install pipeline reuse.

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
