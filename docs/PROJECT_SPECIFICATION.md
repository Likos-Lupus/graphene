# Graphene Project Specification

## Product

Graphene is a Rust library that provides the non-UI engine of a Minecraft: Java Edition launcher.
Hosts such as a CLI, Tauri application, Slint application, or another frontend call Graphene-owned
APIs for metadata resolution, artifact acquisition, instance installation, Java selection,
authentication, launch planning/execution, and lifecycle observation.

Graphene is not itself a desktop UI. Its durable contracts are normalized domain values, operations,
plans, persisted instance state, and structured diagnostics rather than provider responses or host
widgets.

## Scope

The engine owns these product responsibilities:

- explicit engine construction around a caller-selected data root;
- bounded/cancellable operations, progress, events, errors, and diagnostics;
- verified artifact acquisition and immutable cache identity;
- safe managed filesystem layout and transaction/publication primitives;
- normalized Minecraft metadata, inheritance, rules, arguments, libraries, assets, natives, and Java
  requirements;
- provider-neutral component composition for Minecraft and supported loaders;
- deterministic install planning and create-only transactional installation;
- persisted provider-neutral instance/receipt state that supports offline launch planning;
- local and managed Java discovery/selection/probing;
- provider-neutral accounts, Microsoft authentication adapter support, and injected secure secret
  storage;
- direct-argv Minecraft/process execution with bounded lifecycle/output events;
- Fabric, Forge, and NeoForge provider adapters that normalize into shared domain/install models.

The current public behavior is described in [`API.md`](API.md). Evidence-based loader limitations
are in [`LOADER_SUPPORT.md`](LOADER_SUPPORT.md).

## Final target

A complete Graphene backend should let a host manage the full non-UI launcher lifecycle without
reimplementing provider, installation, process, or safety logic. The long-term target includes:

- reliable Vanilla and supported-loader install/launch flows;
- authenticated and offline account workflows;
- managed Java acquisition plus local Java selection;
- mutable instance management with locking, verify, and repair;
- provider-neutral content discovery/install/update;
- normalized modpack import/export pipelines;
- structured diagnostics and repair recommendations;
- reference host integrations proving that the backend remains UI-independent.

Future work must extend the same normalized artifact/plan/transaction boundaries rather than add a
parallel launcher pipeline. The sequence lives in [`ROADMAP.md`](ROADMAP.md).

## High-level bounded contexts

### Core

Typed identifiers, hashes, artifacts, errors, diagnostics, cancellation, operation state/events, and
shared filesystem-neutral primitives. Core does not own provider, storage-policy, network, UI, or
process orchestration.

### Platform

Operating-system normalization and private process/filesystem primitives that require host OS
access. Platform types do not become a second domain model.

### Network

The shared Reqwest/Rustls transport boundary: URL policy, bounded protocol responses, streamed
artifact transfer, retry, concurrency, and verification support. Other crates use Graphene-owned
ports/types instead of Reqwest types.

### Storage

Graphene data-root initialization, containment, immutable cache/materialization paths, staged
publication, runtime/account persistence adapters, and related filesystem safety.

### Minecraft

Provider-neutral Minecraft and loader-component domain models: metadata, rules, Maven coordinates,
resolved runtime state, component graphs, patches, and preparation recipes.

### Providers

Volatile external protocol adapters for Mojang, Microsoft, Java distributions, and loaders. Raw DTOs
stay private and normalize at the boundary.

### Install

Deterministic plans and transaction-safe execution. Install owns domain-side acquisition/tool-runner
ports and never calls providers or the network transport directly.

### Instance

Stable instance identity and schema-versioned provider-neutral persisted launch/install state.

### Java

Java requirements, discovery, probing, compatibility, managed-runtime descriptors, and distribution
ports.

### Authentication

Provider-neutral account/session/secret contracts. Secure storage is a port injected by the host or
distributor; no plaintext fallback is silently enabled.

### Launch

Offline launch-plan construction from committed normalized state plus ephemeral session data, and
the public game-process lifecycle abstraction.

### Service and root facade

Composition and dependency-inversion adapters. The root `graphene` crate re-exports stable
Graphene-owned entry points; service wires implementations without taking ownership of domain
algorithms.

Detailed dependency rules are intentionally not duplicated here; see [
`ARCHITECTURE.md`](ARCHITECTURE.md).

## Supported-state principles

### Explicit identities

Install plans resolve moving/provider-specific selectors to explicit versions before committed
mutation. Base Minecraft identity is stored independently from loader identity. Cache and generated
artifact reuse requires verifiable identity/provenance rather than file existence.

### Transactional mutation

A failed or cancelled operation must not leave a valid-looking committed instance/runtime. Work is
performed in Graphene-managed staging, validated, and published only at a documented point of no
return. Existing committed targets are not casually overwritten.

### Offline launch state

After installation, launch planning must rely on committed normalized state rather than reparsing
provider DTOs or requiring provider/network availability. Authentication/session refresh is a
separate concern and remains ephemeral/secret-aware.

### Provider neutrality

Provider protocols may vary, but stable service/install/launch code consumes Graphene-owned models.
Adding a provider should primarily add an adapter and registration/composition, not a provider
branch through the install or launch pipeline.

### Security before convenience

Untrusted archive paths, symlink escapes, unverifiable artifacts, unsafe repository URLs, hidden
shell execution, secret leakage, and undeclared generated output are rejected rather than accepted
for compatibility convenience. Current residual risks are documented in [
`SECURITY.md`](SECURITY.md).

## Non-goals

Graphene does not aim to:

- embed a UI framework in the engine;
- expose provider DTOs or transport/process handles as stable API;
- own a process-global async runtime or tracing subscriber;
- silently store credentials in plaintext when a secure backend is unavailable;
- execute arbitrary shell/script actions from loader metadata;
- claim JVM/OS sandboxing for trusted verified Java processor bytecode;
- treat the component graph as a general mod/package dependency solver;
- install Fabric API implicitly when Fabric Loader is selected;
- treat legacy Forge metadata support as modern installer execution support;
- rely on the public internet in normal CI tests;
- freeze today's crate/file/dependency layout as an exact architectural snapshot;
- implement future instance/content/modpack work prematurely as part of repository cleanup.

## Completion and evidence

A feature is not considered release-sign-off complete because implementation code or deterministic
fixtures exist. Automated quality gates must pass in a supported Rust environment and required real
provider/runtime smoke procedures must be executed honestly. Current evidence and `NOT RUN` items
are owned by [`VALIDATION.md`](VALIDATION.md).
