# Graphene Architecture

Graphene is a UI-independent launcher engine built as a Cargo workspace of bounded-context crates.
This document owns dependency direction and architectural boundaries. Product scope is in
[`PROJECT_SPECIFICATION.md`](PROJECT_SPECIFICATION.md); code/test/document standards are in
[`ENGINEERING_STANDARDS.md`](ENGINEERING_STANDARDS.md).

## Layer model

```text
host/UI/CLI
    |
    v
root graphene facade
    |
    v
graphene-service  (composition/adapters)
    |
    +-----------------------------------------------+
    |               |               |               |
 install/launch   providers      storage         content
    |               |               |               |
    +-------- domain ports/models --+---------------+
                    |
         minecraft / java / auth / instance
                    |
                   core

network and platform are infrastructure boundaries used only where architecture permits.
```

The diagram is directional guidance, not an exact manifest snapshot. A crate is allowed to have
*fewer* dependencies when a refactor removes an unnecessary edge.

## Current dependency direction

Durable rules:

- `graphene-core` remains infrastructure-independent except for narrow foundational value/serde
  dependencies permitted by the architecture checker.
- UI frameworks do not enter engine crates.
- Reqwest implementation types stay in `graphene-network`.
- Provider DTO-like types stay private to `graphene-providers`.
- `graphene-install` does not depend on providers, network, or service; it owns ports that service
  adapts to verified acquisition and tool execution.
- `graphene-launch` does not depend on auth providers or loader/provider implementations; it
  consumes committed normalized launch state plus an ephemeral session.
- `graphene-minecraft` owns provider-neutral Minecraft/component/patch/preparation domain models and
  does not depend on provider/service/install infrastructure.
- `graphene-java` owns Java domain/ports without depending on provider/network/storage/service.
- `graphene-auth` owns account/auth/secret-store contracts without depending on network/storage/
  service/launch infrastructure.
- `graphene-content` owns provider-neutral content identities, local metadata normalization,
  compatibility evaluation, dependency graph resolution, and non-mutating mutation planning without
  depending on `graphene-providers`, `graphene-network`, or `graphene-service`.
- `graphene-launch` remains unaware of content providers, mod metadata, or remote catalogs.
- Providers may normalize into stable domain types but may not depend on service composition.
- The internal workspace dependency graph must remain acyclic.

`scripts/check_architecture.py` encodes forbidden edges and boundary checks. It intentionally does
not encode equality with the current complete dependency set.

## Root and crate facades

The root `graphene` crate re-exports Graphene-owned public values/services. Every crate root is a
thin facade: module declarations and exports belong there; business implementations belong in
cohesive modules. The checker enforces a stricter line/item rule for crate roots than ordinary
source files.

Provider DTOs, Reqwest responses/builders, archive implementation details, Tokio process handles,
and storage transaction primitives are not root-facade API.

## Artifact acquisition inversion

There is one verified acquisition pipeline. Domain/install code describes an `Artifact` and uses an
installer-owned acquisition port. Service adapts that port to the shared network/cache pipeline.
Provider metadata acquisition follows the same direction through narrow Graphene-owned adapters.

This preserves integrity, cancellation, retry, cache, and error semantics without introducing
`install -> service` or `install -> network` dependencies.

## Transaction and publication boundaries

Installation and managed-runtime creation separate immutable/shared acquisition from isolated
staging. Staged state is validated before a final publication boundary. Cancellation may abort
safely before the seal; once cancellation is sealed for the small non-interruptible commit,
committed success is not retroactively relabeled as cancellation.

Persisted receipts/descriptors contain Graphene-owned normalized identities and managed-relative
paths, not provider DTOs, transient staging paths, host secrets, or absolute data-root paths.

## Minecraft and loader composition

`graphene-minecraft` owns the component graph and ordered `MinecraftVersionPatch` model. The graph
requires one Minecraft base component, structurally rejects conflicting primary loaders, validates
requirements/cycles/bounds, and produces deterministic ordering.

Loader adapters resolve provider-specific discovery/integrity/profile data into exact component
identity, a normalized patch, and optional `ComponentPreparationRecipe`. Install execution consumes
that recipe generically. Launch consumes only committed normalized state and has no loader-family
execution branch.

Forge and NeoForge remain distinct provider adapters. Their modern verified installer profiles may
converge on the shared Forge-family preparation model after provider-specific discovery and
integrity verification.

## Process boundary

Java probes, install tools, and Minecraft are launched by direct executable + argv, never by
constructing a shell command. Platform/process implementation handles remain private. Public launch
exposes Graphene-owned lifecycle/event/result types with bounded output behavior.

Install-tool execution is dependency-inverted: the install domain describes explicit Java/tool/argv
inputs; service selects Java and performs bounded direct execution. Provider/install metadata cannot
silently request arbitrary shell/script/native execution.

## Instance repository, advisory lease model, and desired state

Instance management is unified in one cross-process architecture:

- **Repository**: `InstanceRepository` provides the single authority for loading, validating, and
  updating instance metadata documents (`instance.json`, `.graphene/install.json`,
  `.graphene/lock.json`, `.graphene/config.json`, and `config/instance-defaults.json`).
- **Advisory lease model**: OS-level shared/exclusive file locks backed by persistent carrier files
  at `instances/.locks/<id>.lock`. Carriers are persistent infrastructure and are never deleted on
  unlock.
- **Desired-state lockfile**: `instances/<id>/.graphene/lock.json` persists exact reconstructable
  artifact sources, hashes, sizes, scopes, native extractions, and generated outputs. It serves as
  the single source of truth for verification and repair.
- **Runtime lease ownership**: Launch acquires a shared lease and verifies that the plan's
  `InstanceStateFingerprint` matches current disk state. Upon process spawn, ownership of the shared
  lease transfers into `RunningGame` / background monitor task until process termination, preventing
  concurrent delete, repair, or mutation while a game is running.
- **Deterministic repair orchestration**: Repair is split into planning (deriving a non-mutating
  `RepairPlan` from desired state vs. filesystem observations) and execution (applying actions via
  existing install acquisition and materialization primitives under an exclusive lease).

## Content management and desired-state convergence

Content management (mods and remote catalogs) is fully converged on the existing instance engine and
verified acquisition pipeline:

- **Strict layer separation**: `graphene-content` defines provider-neutral models, metadata parsers
  (Fabric, Forge, NeoForge, Legacy), and dependency resolution algorithms. `graphene-providers` owns
  Modrinth/CurseForge DTOs, endpoint adapters, and secret redaction. `graphene-service` orchestrates
  leases, verified pre-acquisition, staging, and journaled transactions.
- **Single desired-state authority**: Managed mods are declared directly in `lock.json` schema 2
  under `content` and `artifacts`. There is no separate `.graphene/content.json` or parallel
  authority.
- **Physical inventory vs. desired state vs. catalog identity**: Physical inventory
  (`.minecraft/mods`)
  is discovered offline; desired state reflects what Graphene manages; catalog identity is explicit
  provenance. Unmanaged files remain untouched by normal verification and are never bulk-deleted.
- **Journaled execution and crash recovery**: Content mutations execute under an exclusive lease
  with stale-state protection (`InstanceStateFingerprint` + `ContentInventoryFingerprint`), staged
  publication, old-file quarantine, and a durable journal (`.graphene/content-journal.json`). The
  atomic lockfile write is the authoritative point-of-no-return commit marker. Interrupted
  transactions recover idempotently to the committed desired state.
- **Verification and repair convergence**: Managed mods are verified via existing quick and full
  instance verification. Corrupted or missing managed mods are repaired through existing
  provider-neutral Phase 4 repair primitives without requiring a live content provider connection.

## Storage and archive boundaries

`graphene-storage` owns the data root, containment, and publication policy. Paths crossing managed
boundaries are validated lexically and, where needed, canonically against symlink escapes.

The filesystem-neutral bounded ZIP/DEFLATE byte codec lives once in `graphene-core::archive` so
security-sensitive parsing is not duplicated. Install owns archive extraction/path/write policy;
providers own selective verified installer-JAR inspection and provider-specific limits.

## Authentication and secrets

`graphene-auth` owns provider-neutral account/session/secret contracts. Provider protocol adapters
normalize external identity responses. Secret persistence is an injected `SecretStore`; the default
unavailable store fails rather than silently falling back to plaintext credentials.

Launch sessions are ephemeral and secret-bearing values use redacting wrappers. Persisted instance
state does not contain account access/refresh tokens.

## Managed Java

Java compatibility/discovery/probe models live in `graphene-java`; service composes local selection
with committed managed runtime inventory and a distribution-provider port. Managed runtime archives
are verified before extraction, staged, probed, descriptor-validated, and published atomically.

The reference distribution adapter is an implementation behind the port, not an architectural
requirement for every distributor.

## Diagnostics bounded context

`graphene-diagnostics` owns provider-neutral diagnostic models, bounded text/crash parsing,
deterministic secret/path redaction, evidence correlation, confidence classification, and
non-mutating recommendation derivation. It may depend on stable Graphene-owned domain values from
`graphene-core`, `graphene-instance`, and `graphene-content`; it never depends on
`graphene-service`, providers, networking, storage, authentication, launch, modpack, install,
Minecraft/platform implementations, or UI/host crates. `graphene-instance`, `graphene-content`,
`graphene-java`, and `graphene-launch` do not gain a reverse dependency on diagnostics.

Filesystem access, instance leases, Java probing/selection, content scanning, and service
composition stay in `graphene-service`: `DiagnosticService` orchestrates existing operations
(verification, content scan, Java selection) under the shared instance lease and passes normalized
snapshots/bytes into the diagnostics crate. Diagnosis is read-only and introduces no second repair
pipeline; recommendations converge on existing verify/repair, Java, and content planning/execution
APIs. `scripts/check_architecture.py` treats `graphene-diagnostics` as an engine crate and encodes
these forbidden edges.

## Reference host packages

Reference hosts are outward consumers of the root `graphene` facade, not part of the engine. They
live in a separate Cargo workspace under `apps/` so that Tauri/graphics system dependencies never
enter the engine workspace gates or its cross-platform CI matrix:

```text
apps/
  graphene-reference-host-support/   # host-only config, secure-store, tracing, operation/run bridges
  graphene-cli/                      # reference CLI
  graphene-tauri/                    # Vite/TypeScript frontend + src-tauri (graphene-tauri-host)
```

Durable rules:

- no engine package depends on a host package;
- host packages depend on the root `graphene` facade, never on internal engine crates;
- `tauri` is permitted only in `graphene-tauri-host`; `slint` is never permitted;
- provider DTOs and Reqwest do not migrate into hosts;
- host wire/JSON DTOs are app-local compatibility contracts and are never added to `graphene-core`;
- hosts never execute Java/Minecraft through shell strings and never call provider HTTP endpoints
  directly;
- host entry points (`lib.rs`, `main.rs`, Tauri command modules) remain thin and bounded so they do
  not become god files.

`scripts/check_architecture.py` distinguishes engine packages from host packages and enforces these
rules without weakening the engine UI-dependency ban. `scripts/check_hygiene.py` applies the
production-size and phase-naming rules to host Rust and TypeScript sources as well.

## Architecture checks

`scripts/check_architecture.py` should fail for durable violations such as:

- forbidden internal dependency edges or cycles;
- UI dependencies inside the engine, or Tauri outside the designated host;
- engine packages depending on host packages, or hosts reaching around the facade;
- Reqwest escaping the network boundary into engine or host packages;
- public/provider DTO leakage;
- shell execution patterns in protected domain/install/host code;
- duplicated bounded archive codecs;
- missing/thick/business-logic crate roots or host entry points.

It must not fail because a legitimate refactor removes a dependency, module, or file. Repository
hygiene conventions are checked separately by `scripts/check_hygiene.py`.

## Modpack bounded context

`graphene-modpack` is the single normalized modpack context: source/metadata/runtime/file/seed
models, deterministic format detection, per-format adapters (Modrinth, CurseForge, MultiMC, Graphene
pack v1, generic), and strict archive indexing live there. Format DTOs are private to the crate,
mirroring the provider-DTO rule. The crate depends only on `graphene-core`; it never depends on
services, networking, storage, platform, or providers.

Inverted acquisition is preserved for packs: `graphene-install` defines a generic seed-layer
extension (`SeedArchiveLayer`) with no modpack-format knowledge; `graphene-service` adapts
normalized packs onto it. The service layer owns all provider interaction — CurseForge entries in a
normalized pack are exact `ContentVersionRef` references resolved through the existing
`ContentProvider` boundary, never a parallel download path.

Import/export share one pipeline: detection → normalization → composite plan (fingerprinted,
validated, mutation-free) → staged transaction with a single create-only publication. Export reads
committed desired state (lockfile + cache), snapshots verified bytes with staleness checks, and
validates its own archive against the import contract before publishing deterministically.
