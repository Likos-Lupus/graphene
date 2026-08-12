# Graphene Implementation Roadmap

This roadmap defines implementation order and measurable phase exits.

## Phase 0 — Foundation

> Detailed implementation specification: [
`PHASE_0_IMPLEMENTATION_PLAN.md`](PHASE_0_IMPLEMENTATION_PLAN.md)

### Build

- Cargo workspace;
- root `graphene` facade;
- `graphene-core`;
- `graphene-platform`;
- `graphene-network`;
- operation/event/progress model;
- error/diagnostic model;
- cancellation;
- storage root and initial layout.

### Exit

- compilation on supported development platforms;
- artifact download fixture with hash verification;
- nested progress demonstration;
- cancellation leaves no committed partial output;
- no UI dependency.

## Phase 1 — Vanilla End-to-End

### Build

- Mojang version manifest adapter;
- normalized Minecraft metadata;
- inheritance/rules/libraries/assets/natives;
- `ResolvedMinecraft`;
- `InstallRequest`;
- `InstallPlan`;
- transaction executor;
- Java discovery/probe/selection;
- `LaunchRequest`;
- `LaunchPlan`;
- process runner.

### Exit

From a fresh Graphene directory, install one Vanilla version and launch it through the library.

This is the first release-worthy technical milestone.

## Phase 2 — Authentication and Managed Java

**Current repository status (2026-08-11): implementation candidate.** Provider-neutral accounts,
Microsoft fixture/protocol adapter, offline identities, separate secret-store port, managed Temurin
resolution, staged managed-runtime installation, diagnostics, and architecture guards are present.
Phase 2 is not marked complete: the Rust quality gates cannot run in the current sandbox, the Phase
1 real Vanilla smoke remains unpassed, no bundled OS credential-vault adapter exists, and the two
Phase 2 real smokes remain `NOT RUN`.

### Build

- Microsoft auth;
- offline account;
- session refresh;
- secure secret store;
- managed Java provider;
- Java diagnostics.

### Exit

Both authenticated and offline profiles feed the same launch pipeline; no secrets appear in logs.

## Phase 3 — Loader Components

**Current worktree status:** implementation candidate. Component/patch domains, loader registry,
Fabric plus Forge/NeoForge normalization, staged Java preparation, generated-output verification,
and receipt schema 2 are present. Cargo validation and real loader smoke sign-off remain pending;
see the Phase 3 API/security/support/smoke documents.

### Build

- component graph;
- Fabric provider;
- NeoForge provider;
- Forge provider;
- loader compatibility metadata.

### Exit

All supported loaders converge into `ResolvedMinecraft`; launch orchestration contains no
loader-specific branches.

## Phase 4 — Instance Engine

### Build

- instance repository;
- create/delete/rename/clone;
- global/per-instance config;
- locking;
- lockfile;
- verify/repair.

### Exit

Repair is a planned transaction based on lockfile/filesystem diff.

## Phase 5 — Content

### Build

- local mod scanner;
- normalized project/version/file/dependency;
- Modrinth;
- CurseForge;
- hash lookup;
- install/update;
- dependency checks.

### Exit

Service APIs remain provider-neutral and install/update goes through the artifact/install pipeline.

## Phase 6 — Modpacks

### Build

- `.mrpack`;
- CurseForge pack;
- Prism/MultiMC import;
- generic local/URL archive;
- Graphene pack format/export.

### Exit

Every format normalizes to a common pack/install model and passes archive security tests.

## Phase 7 — Diagnostics

### Build

- verifier;
- Java diagnostics;
- crash report collector;
- log parser;
- secret redaction;
- repair recommendations.

### Exit

Hosts can present meaningful structured diagnostics without string matching.

## Phase 8 — Host Integration

### Build

- reference CLI;
- Tauri or Slint adapter;
- second host adapter where useful.

### Exit

At least two host types use the same backend logic.

## Priority Policy

P0 work is anything required to preserve the install-to-launch backbone.

P1 work brings Graphene to a full modern launcher backend.

P2 work is differentiation, convenience, or ecosystem breadth and must not destabilize P0/P1
boundaries.
