# Graphene Project Definition and Architecture Specification

**Status:** Architecture baseline  
**Version:** 0.1  
**Language:** English  
**Primary implementation language:** Rust 2024  
**Project type:** UI-independent Minecraft: Java Edition launcher engine/library

---

## 1. Purpose of This Document

This document defines the initial engineering direction, the intended final product, the
architectural model, the bounded contexts, the dependency rules, and the delivery milestones for *
*Graphene**.

It is intended to be the baseline against which future implementation decisions are evaluated. It is
not a UI specification, and it is not a feature wishlist. It answers five concrete questions:

1. What is Graphene?
2. What must the first implementation prove?
3. What should the mature project eventually provide?
4. How is the system divided into modules and bounded contexts?
5. What dependencies and responsibilities are explicitly forbidden?

Any substantial architectural change should either remain compatible with this document or be
recorded as an Architecture Decision Record (ADR).

---

## 2. Product Definition

Graphene is a **UI-independent, provider-extensible Minecraft: Java Edition launcher engine**
written in Rust.

Its purpose is to provide the complete non-UI backend required by a modern desktop launcher,
including:

- Minecraft version resolution;
- installation, verification, repair, and update planning;
- Java runtime discovery, selection, and managed runtimes;
- account authentication and session management;
- isolated instance management;
- mod loader support;
- local and remote content management;
- modpack import and export;
- launch planning and process lifecycle management;
- diagnostics and crash-context collection;
- persistent configuration and secure secret handling;
- unified long-running operation progress and cancellation.

Graphene is designed to be consumed by multiple front ends without changing launcher logic:

```text
                    Graphene Engine
                          |
          +---------------+---------------+
          |               |               |
       Tauri UI         Slint UI          CLI
          |               |               |
          +---------------+---------------+
                          |
                 Other future hosts
```

Graphene must remain usable without any graphical framework.

---

## 3. Product Statement

The project should be understood as:

> A reusable Minecraft launcher engine that can install, manage, verify, repair, and launch isolated
> Minecraft instances through a stable Rust API while keeping UI concerns, third-party API DTOs,
> network implementation details, and platform-specific behavior outside the domain model.

Graphene is **not** merely a command-line builder and is **not** a collection of helper functions
around Minecraft metadata.

---

## 4. Starting Direction

The first engineering goal is deliberately narrow.

### 4.1 First Vertical Slice

The first working slice must prove this complete path:

```text
Official Minecraft metadata
        |
        v
Version resolution
        |
        v
InstallRequest
        |
        v
InstallPlan
        |
        v
Artifact download + verification
        |
        v
Materialized isolated instance
        |
        v
Java discovery / selection
        |
        v
LaunchRequest
        |
        v
LaunchPlan
        |
        v
Minecraft process
```

### 4.2 First Milestone Definition

The first meaningful milestone is complete when Graphene can:

1. initialize an empty Graphene data directory;
2. query and parse the official Minecraft version manifest;
3. resolve one supported Vanilla release into a normalized internal model;
4. calculate all required artifacts before changing the final instance state;
5. download and verify the client, libraries, assets, natives, and logging configuration where
   applicable;
6. create a new isolated instance transactionally;
7. discover local Java runtimes;
8. select a runtime compatible with the resolved Minecraft requirement;
9. generate a complete `LaunchPlan` without starting the game;
10. execute that plan and stream process output and lifecycle events;
11. cancel long-running installation work safely;
12. repeat verification without redownloading valid artifacts.

### 4.3 What Must Not Be Built First

The following are intentionally postponed until the Vanilla vertical slice is stable:

- CurseForge search;
- Modrinth browsing UI;
- modpack catalogs;
- skin management;
- launcher self-update;
- dynamic plugins;
- rich crash diagnosis;
- Tauri commands;
- Slint controllers;
- broad loader support.

This prevents attractive peripheral features from becoming the architecture before the core launcher
engine exists.

---

## 5. Intended Final Product

The mature Graphene project should provide a backend comparable in breadth to modern full-featured
Minecraft launchers while retaining a reusable library architecture.

### 5.1 Final Functional Areas

The mature product should support the following areas.

#### Instance Management

- create, delete, clone, rename, group, and inspect instances;
- isolate per-instance `.minecraft` content;
- global configuration with per-instance overrides;
- instance locking and concurrent-operation protection;
- instance metadata such as icon, notes, tags, last launch time, and play time;
- reproducible instance state through a Graphene lockfile;
- verification and repair against declared state.

#### Minecraft Resolution

- official version manifest parsing;
- release, snapshot, old beta, and old alpha metadata;
- version JSON inheritance;
- legacy and modern argument formats;
- OS, architecture, and feature rule evaluation;
- Maven coordinate handling;
- classpath calculation;
- native classifier selection and extraction;
- asset index/object resolution;
- logging configuration;
- Java version requirements.

#### Loader and Component System

Initial priority:

- Vanilla;
- Fabric;
- NeoForge;
- Forge.

Later support:

- Quilt;
- Legacy Fabric;
- LiteLoader;
- OptiFine;
- Cleanroom or future loaders where feasible.

Loaders must be represented as components/providers rather than hard-coded branches in launcher
orchestration.

#### Java Runtime Management

- discover installed Java runtimes;
- probe version, architecture, vendor, and executable validity;
- model Java requirements as constraints;
- select a compatible runtime;
- allow global and per-instance overrides;
- optionally install launcher-managed runtimes;
- produce diagnostics when no compatible runtime exists.

#### Authentication

- Microsoft account support;
- token refresh;
- Minecraft profile and ownership checks where required;
- offline accounts;
- third-party Yggdrasil/authlib-injector support;
- multiple accounts;
- default and per-instance account selection;
- secure secret storage;
- profile, skin, and cape metadata where supported.

#### Content

- local mod enumeration;
- enable/disable state;
- metadata and hash extraction;
- normalized online project/version/file/dependency model;
- Modrinth integration;
- CurseForge integration;
- update lookup;
- dependency resolution;
- duplicate and compatibility checks;
- resource pack, shader pack, data pack, and world management.

#### Modpacks

- Modrinth `.mrpack`;
- CurseForge packs;
- Prism/MultiMC-compatible imports;
- HMCL/other formats where support is practical;
- generic local/URL archive import;
- normalized internal pack manifest;
- export to selected formats;
- Graphene-native reproducible pack format.

#### Installation and Repair

- transaction-based installation;
- artifact planning before mutation;
- staging directories;
- hash and size verification;
- retry, timeout, cancellation, and resumable downloads;
- deduplicated downloads and caching;
- repair through the same plan/executor infrastructure;
- rollback or cleanup on failure.

#### Launching and Process Lifecycle

- deterministic launch planning;
- explicit Java executable;
- JVM arguments;
- classpath;
- main class;
- game arguments;
- environment variables;
- working directory;
- natives directory;
- wrapper/pre-launch/post-exit hooks as explicit optional features;
- process ID and handle;
- stdout/stderr events;
- graceful stop and forced termination;
- exit status and play-time accounting.

#### Diagnostics

- installation verification;
- Java incompatibility;
- missing/corrupt artifacts;
- loader mismatch;
- basic mod conflict evidence;
- crash report collection;
- `latest.log`/stdout context;
- redaction of access tokens and other secrets;
- structured diagnostic codes independent of UI language.

#### Platform and Persistence

- Windows, Linux, and macOS;
- x86_64 and aarch64 where upstream Minecraft/Java support allows;
- launcher configuration;
- instance configuration;
- account metadata;
- secure secret storage;
- schema versioning and migrations;
- filesystem as the recoverable source of truth;
- cache/database as disposable acceleration layers.

---

## 6. Final Deliverable Definition

Graphene is considered a complete backend platform when the following statement is true:

> A host application can build a full Minecraft launcher by depending on the `graphene` facade,
> presenting its own UI, and translating Graphene requests, results, operations, events, and
> diagnostics without implementing Minecraft installation, authentication, content resolution, Java
> selection, launch command construction, or process management itself.

### 6.1 Expected Repository-Level Deliverables

A mature repository should contain:

```text
graphene/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
├── src/
│   └── lib.rs
├── crates/
│   ├── graphene-core/
│   ├── graphene-platform/
│   ├── graphene-network/
│   ├── graphene-minecraft/
│   ├── graphene-auth/
│   ├── graphene-java/
│   ├── graphene-content/
│   ├── graphene-instance/
│   ├── graphene-pack/
│   ├── graphene-providers/
│   ├── graphene-install/
│   ├── graphene-launch/
│   ├── graphene-diagnostics/
│   ├── graphene-storage/
│   └── graphene-service/
├── tests/
│   ├── fixtures/
│   └── integration/
├── fuzz/
├── examples/
└── docs/
```

The root crate remains the stable consumer-facing facade.

---

## 7. Architectural Principles

The architecture is governed by the following principles.

### 7.1 Stable Domain, Replaceable Adapters

Minecraft concepts, instance state, content identity, install plans, launch plans, diagnostics, and
operations belong to Graphene's stable model.

HTTP APIs, provider JSON formats, keyrings, filesystems, and platform process APIs are replaceable
implementation details.

### 7.2 Plan Before Execute

Complex operations must be represented before side effects occur.

Installation:

```text
InstallRequest
     |
     v
Resolver
     |
     v
InstallPlan
     |
     v
Executor
     |
     v
Transactional commit
```

Launching:

```text
LaunchRequest
     |
     v
LaunchResolver
     |
     v
LaunchPlan
     |
     v
ProcessRunner
```

### 7.3 UI Independence

No Graphene domain crate may depend on Tauri, Slint, a windowing toolkit, or UI callback types.

Graphene publishes typed data and typed events. Hosts decide how to render them.

### 7.4 Provider Isolation

Third-party API DTOs must be converted at the provider boundary.

For example:

```text
Modrinth DTO ----+
                 +--> Normalizer --> ContentProject / ContentVersion
CurseForge DTO --+
```

A service or UI must never require a Modrinth response type to perform ordinary content operations.

### 7.5 Recoverable Persistence

The filesystem representation of user-owned state must be sufficient to reconstruct instances and
configuration.

SQLite may be used for indexing, caching, history, and acceleration, but deleting the cache database
must not destroy the user's instances.

### 7.6 Structured Failure

Public APIs must return structured errors and diagnostics. UI behavior must never depend on parsing
human-readable error strings.

### 7.7 Long Operations Are First-Class

Downloads, installation, repair, modpack import, and similar work must be:

- asynchronous;
- cancellable;
- observable;
- retry-aware;
- represented by stable operation identifiers;
- able to report nested progress.

---

## 8. Core Abstractions

### 8.1 Artifact

`Artifact` is the normalized description of anything that must be obtained, verified, optionally
extracted, and materialized.

Conceptual model:

```rust
pub struct Artifact {
    pub id: ArtifactId,
    pub kind: ArtifactKind,
    pub sources: Vec<ArtifactSource>,
    pub hashes: HashSet,
    pub size: Option<u64>,
    pub target: ArtifactTarget,
    pub extraction: Option<ExtractionSpec>,
}
```

Potential kinds include:

```text
MinecraftClient
Library
Native
Asset
LoggingConfig
Loader
Mod
ResourcePack
ShaderPack
JavaRuntime
External
```

The artifact pipeline must be reused by installation, repair, loader installation, content
installation, modpacks, and managed Java.

---

### 8.2 Component Graph

Minecraft and loaders should resolve into a graph of components rather than a chain of
loader-specific `if` statements.

Conceptual model:

```rust
pub struct Component {
    pub uid: ComponentUid,
    pub version: ComponentVersion,
    pub order: i32,
    pub requires: Vec<ComponentRequirement>,
    pub conflicts: Vec<ComponentConflict>,
    pub metadata_patch: VersionPatch,
}
```

Resolution produces a normalized result:

```rust
pub struct ResolvedMinecraft {
    pub main_class: String,
    pub libraries: Vec<ResolvedLibrary>,
    pub natives: Vec<ResolvedNative>,
    pub jvm_args: Vec<Argument>,
    pub game_args: Vec<Argument>,
    pub assets: ResolvedAssets,
    pub java_requirement: JavaRequirement,
}
```

This keeps new loaders from changing the launch core.

---

### 8.3 InstallPlan

All non-trivial writes to an instance must be represented as an installation plan.

A plan may include:

- metadata acquisition;
- downloads;
- verification;
- extraction;
- filesystem materialization;
- configuration writes;
- lockfile writes;
- commit operations.

Execution uses a staging area such as:

```text
instances/<id>/
instances/.graphene-staging-<operation-id>/
```

Only a successful plan may commit to the final instance state.

Failure or cancellation must leave the original committed state valid.

---

### 8.4 LaunchPlan

Launch resolution and process execution are separate operations.

A `LaunchPlan` should fully describe:

- Java executable;
- working directory;
- environment;
- JVM arguments;
- classpath;
- main class;
- game arguments;
- natives path;
- optional wrapper;
- optional pre-launch command;
- optional post-exit command;
- redacted debugging context.

This allows:

- dry-run launch inspection;
- snapshot tests;
- CLI output;
- UI debugging;
- reproducible bug reports;
- process execution through multiple hosts.

---

### 8.5 Operation Model

All long-running work uses one operation model.

Conceptual data:

```rust
pub struct OperationEvent {
    pub operation_id: OperationId,
    pub parent_id: Option<OperationId>,
    pub kind: OperationKind,
    pub stage: OperationStage,
    pub state: OperationState,
    pub progress: Option<Progress>,
}
```

Nested progress should support structures such as:

```text
Install Fabric Instance
├── Resolve metadata        done
├── Download                62%
│   ├── client              done
│   ├── libraries           73%
│   └── assets              58%
├── Verify                  waiting
└── Commit                  waiting
```

---

## 9. Workspace and Bounded Contexts

The target workspace uses approximately fifteen bounded-context crates. The goal is not maximum
crate count; the goal is explicit ownership and dependency direction.

### 9.1 `graphene-core`

**Owns**

- stable IDs;
- common hash types;
- artifact primitives;
- capability flags;
- operations and progress;
- event primitives;
- diagnostic primitives;
- base error codes and shared value objects.

**Must not own**

- Minecraft version parsing;
- HTTP clients;
- provider DTOs;
- UI types;
- filesystem layout policy.

**Dependency rule:** should be near the bottom of the dependency graph.

---

### 9.2 `graphene-platform`

**Owns**

- OS and architecture detection;
- platform directories;
- filesystem capabilities;
- process primitives;
- system memory/CPU information;
- keyring abstraction;
- recycle bin/trash adapters where implemented.

**Must not own**

- Minecraft rules;
- account policy;
- instance business logic;
- content APIs.

---

### 9.3 `graphene-network`

**Owns**

- HTTP client abstraction;
- proxy support;
- retry/backoff;
- timeouts;
- rate limiting;
- HTTP caching/ETag support;
- mirror/source selection;
- download manager;
- resumable download;
- verification handoff;
- download deduplication.

**Must not own**

- Mojang DTO normalization;
- Modrinth/CurseForge domain logic;
- instance layout decisions.

---

### 9.4 `graphene-minecraft`

**Owns**

- official Minecraft metadata models;
- version manifests;
- inheritance and merge semantics;
- rules;
- arguments;
- Maven coordinates;
- libraries;
- assets;
- natives;
- component graph;
- normalized resolved Minecraft model.

**Must not own**

- network transport;
- Modrinth/CurseForge;
- account login;
- UI;
- process spawning.

---

### 9.5 `graphene-auth`

**Owns**

- account identities;
- authentication sessions;
- account profiles;
- authentication provider interfaces;
- Microsoft/offline/Yggdrasil/authlib-injector normalized behavior;
- session refresh semantics;
- skin/cape domain information where appropriate.

**Must not own**

- OS keyring implementation details;
- UI browser windows;
- Minecraft installation.

Interactive authentication is expressed as typed requests/events, such as opening a browser URL or
displaying a device code.

---

### 9.6 `graphene-java`

**Owns**

- `JavaRuntime`;
- runtime probing;
- discovery;
- requirements and constraint solving;
- runtime selection;
- managed-runtime metadata.

**Must not own**

- direct UI prompts;
- Minecraft version parsing internals;
- arbitrary download implementation.

---

### 9.7 `graphene-content`

**Owns**

- normalized content project/version/file models;
- dependency types;
- loader/game-version filters;
- content provider interfaces;
- local content inspection;
- update resolution.

**Must not own**

- Modrinth DTOs outside adapters;
- CurseForge DTOs outside adapters;
- instance transaction execution;
- UI search state.

---

### 9.8 `graphene-instance`

**Owns**

- instance identity and lifecycle;
- instance configuration;
- layout rules;
- repositories;
- locking;
- cloning;
- metadata;
- state transitions.

**Must not own**

- provider HTTP calls;
- Java probing implementation;
- launch argument construction;
- UI behavior.

---

### 9.9 `graphene-pack`

**Owns**

- normalized pack manifest;
- pack format detection;
- pack importer/exporter interfaces;
- conversion from supported pack formats into a normalized install specification.

**Must not own**

- a second installation engine;
- provider-specific content models beyond import adapters;
- UI import dialogs.

All pack formats must converge into the common installation pipeline.

---

### 9.10 `graphene-providers`

**Owns**

- Mojang API adapter;
- Fabric/Forge/NeoForge/Quilt metadata adapters;
- Modrinth adapter;
- CurseForge adapter;
- Java distribution adapters;
- provider registry;
- mapping between external DTOs and Graphene domain models.

**Must not own**

- core domain policy;
- persistent UI state;
- direct installation orchestration.

This is intentionally the most volatile crate group.

---

### 9.11 `graphene-install`

**Owns**

- `InstallRequest`;
- `InstallPlan`;
- plan resolution;
- execution;
- transaction/staging behavior;
- repair plans;
- commit/rollback semantics.

**Must not own**

- provider DTOs;
- UI progress controls;
- arbitrary process spawning.

---

### 9.12 `graphene-launch`

**Owns**

- `LaunchRequest`;
- `LaunchPlan`;
- classpath construction;
- argument substitution;
- launch environment;
- launch hooks;
- process lifecycle abstraction for Minecraft runs.

**Must not own**

- Microsoft login;
- provider HTTP calls;
- UI notifications.

---

### 9.13 `graphene-diagnostics`

**Owns**

- verifier orchestration;
- Java diagnostics;
- mod/loader evidence;
- crash report collection;
- log parsing;
- secret redaction;
- structured diagnostic results.

**Must not own**

- localized user-facing prose;
- destructive automatic repair without an explicit plan.

---

### 9.14 `graphene-storage`

**Owns**

- serialization schemas;
- launcher config persistence;
- instance metadata persistence;
- account metadata persistence;
- secret-store interface wiring;
- cache/database schemas;
- migrations.

**Must not own**

- domain decisions about which Minecraft version to choose;
- UI settings presentation;
- provider business logic.

---

### 9.15 `graphene-service`

**Owns**

- construction and dependency wiring;
- high-level account/instance/content/java/install/launch/diagnostic services;
- application-level orchestration;
- event bus integration.

This is the composition layer that may coordinate multiple bounded contexts.

It should not duplicate their internal logic.

---

### 9.16 Root `graphene` Crate

The root crate is the stable public facade.

Consumers should normally need only:

```rust
use graphene::{Graphene, GrapheneBuilder};
```

Expected public surface:

```rust
graphene.accounts()
graphene.instances()
graphene.minecraft()
graphene.java()
graphene.content()
graphene.install()
graphene.launch()
graphene.diagnostics()
```

The root crate must contain minimal or no business logic.

---

## 10. Dependency Direction

The architecture must remain a directed acyclic graph.

Conceptually:

```text
                         graphene
                            |
                    graphene-service
                            |
        +-------------------+-------------------+
        |                   |                   |
 graphene-install    graphene-launch    graphene-diagnostics
        |                   |                   |
        +--------+----------+---------+---------+
                 |                    |
          Domain contexts       Supporting contexts
                 |                    |
                 +---------+----------+
                           |
                      graphene-core
```

Concrete adapters sit outside stable domain policy:

```text
graphene-providers
      |
      +--> graphene-minecraft
      +--> graphene-content
      +--> graphene-auth
      +--> graphene-java
      +--> graphene-network
```

### 10.1 Allowed Dependency Philosophy

Higher-level orchestration may depend on lower-level domain interfaces.

Adapters may depend on domain types in order to normalize external data.

Domain crates must not depend on concrete external providers.

### 10.2 Forbidden Dependency Examples

The following dependencies are architectural violations:

```text
graphene-minecraft -> tauri
graphene-instance  -> slint
graphene-content   -> Modrinth response DTO
graphene-launch    -> Microsoft OAuth HTTP client
graphene-auth      -> UI browser window implementation
graphene-core      -> reqwest
graphene-service   -> provider-specific JSON parsing
```

---

## 11. Provider Capability Model

Provider selection must be capability-driven rather than based on chains of provider-name
comparisons.

Conceptually:

```rust
bitflags::bitflags! {
    pub struct ContentCapabilities: u64 {
        const SEARCH = 1 << 0;
        const HASH_LOOKUP = 1 << 1;
        const DEPENDENCY_GRAPH = 1 << 2;
        const UPDATE_LOOKUP = 1 << 3;
        const DIRECT_DOWNLOAD = 1 << 4;
        const MODPACKS = 1 << 5;
        const RESOURCE_PACKS = 1 << 6;
        const SHADERS = 1 << 7;
    }
}
```

Provider identity is normalized:

```rust
pub struct ContentId {
    pub provider: ProviderId,
    pub project: String,
}
```

The domain should not grow fields such as:

```text
modrinth_project_id
curseforge_project_id
future_provider_project_id
```

for every integration.

---

## 12. Storage Model

Recommended data layout:

```text
graphene-data/
├── config/
│   └── graphene.json
├── instances/
│   └── <instance-id>/
│       ├── instance.json
│       ├── graphene.lock.json
│       ├── .minecraft/
│       └── .graphene/
│           ├── state.json
│           ├── install.json
│           └── locks/
├── shared/
│   ├── libraries/
│   ├── assets/
│   ├── runtimes/
│   └── metadata/
├── cache/
│   ├── http/
│   ├── downloads/
│   └── objects/
├── database/
│   └── cache.sqlite
└── logs/
```

### 12.1 Source-of-Truth Rule

**Files are authoritative; SQLite is an index/cache.**

At minimum, `instance.json` and `graphene.lock.json` must preserve enough information to rebuild and
verify an instance.

A destroyed cache database may reduce performance, but it must not erase user-owned launcher state.

---

## 13. Graphene Lockfile

Graphene should maintain a reproducible lockfile for installed state.

Example conceptual contents:

```text
Minecraft:
  version: 1.x.x

Components:
  - vanilla
  - fabric-loader <version>

Artifacts:
  - identity: minecraft-client
    hash: ...
  - identity: library:...
    hash: ...
  - identity: mod:...
    provider: modrinth
    project: ...
    version: ...
    hash: ...

Java:
  requirement: ">= 21, < 22"
```

The lockfile enables deterministic verification:

```text
Lockfile + Filesystem
        |
        v
      Diff
        |
        +--> Missing
        +--> Modified
        +--> Corrupted
        +--> Unexpected
        +--> Outdated
        |
        v
   RepairPlan
```

Repair must reuse the ordinary install executor.

---

## 14. Transaction and Filesystem Boundary

Installation, loader changes, pack import, and repair are mutation-heavy operations and must not
write directly into committed state while resolution is incomplete.

Required semantics:

1. acquire instance mutation lock;
2. resolve desired state;
3. create a plan;
4. create staging area;
5. fetch and verify artifacts;
6. extract/materialize into staging;
7. write metadata and lockfile;
8. validate staged result;
9. atomically commit where supported;
10. clean temporary state;
11. release lock.

Cancellation or failure before commit must preserve the previous committed state.

Where atomic directory replacement is not available, Graphene must use the safest platform-specific
equivalent and record recovery information.

---

## 15. Security Boundaries

Graphene processes untrusted remote content and archives. Security is therefore an architectural
concern, not a late hardening task.

The implementation must defend against:

- archive path traversal (`../`);
- absolute archive paths;
- unsafe symbolic links;
- decompression bombs;
- malformed or oversized metadata;
- malformed JSON/NBT/JAR input;
- redirect abuse;
- hash mismatch;
- accidental secret logging;
- shell injection;
- unsafe command interpolation.

### 15.1 Process Construction Rule

Minecraft execution must use argument arrays:

```rust
Command::new(java)
.arg("-Xmx4G")
.arg(...)
```

Graphene must not construct an untrusted shell string and pass it to a shell.

If custom shell hooks are offered, they must be an explicit opt-in capability with separate
documentation and risk boundaries.

### 15.2 Secret Rule

Access tokens and refresh tokens must not be stored in normal instance configuration.

Preferred hierarchy:

```text
OS keyring
   |
   +--> encrypted fallback secret store when keyring is unavailable
```

All persistent logs and exported diagnostic bundles must pass through a secret redaction layer.

---

## 16. Error and Diagnostic Contract

The public API must not expose a generic string-only error contract.

Conceptually:

```rust
pub struct GrapheneError {
    pub code: ErrorCode,
    pub kind: ErrorKind,
    pub context: ErrorContext,
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}
```

Example stable codes:

```text
NETWORK_TIMEOUT
NETWORK_TLS
HASH_MISMATCH
MINECRAFT_METADATA_INVALID
MINECRAFT_LIBRARY_MISSING
JAVA_NOT_FOUND
JAVA_INCOMPATIBLE
AUTH_EXPIRED
AUTH_REJECTED
CONTENT_DEPENDENCY_UNSATISFIED
INSTALL_CANCELLED
INSTALL_DISK_FULL
LAUNCH_PROCESS_FAILED
```

Diagnostics are separate from execution errors:

```rust
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: DiagnosticSeverity,
    pub parameters: DiagnosticParameters,
}
```

Graphene should provide data such as:

```text
JAVA_VERSION_TOO_OLD
found = 17
required = 21
```

The host application is responsible for localization and presentation.

---

## 17. UI and Host Boundary

Graphene must never require a specific host framework.

### 17.1 Tauri Host

```text
Tauri command/controller
        |
        v
graphene::Graphene
```

### 17.2 Slint Host

```text
Slint controller/model
        |
        v
graphene::Graphene
```

### 17.3 CLI Host

```text
CLI command
    |
    v
graphene::Graphene
```

Host responsibilities include:

- rendering;
- localization;
- dialogs;
- notifications;
- browser/window behavior;
- platform UI lifecycle;
- converting host input into Graphene requests;
- presenting Graphene results and events.

Graphene responsibilities end at typed application behavior.

---

## 18. Public API Direction

The intended facade should be small, coherent, and difficult to misuse.

Example:

```rust
let graphene = Graphene::builder(root)
.build()
.await?;

let versions = graphene
.minecraft()
.versions()
.await?;

let operation = graphene
.instances()
.install(
InstallRequest::new("Vanilla Test")
.minecraft("1.x.x")
)
.await?;

let plan = graphene
.launch()
.plan(instance_id, account_id)
.await?;

let process = graphene
.launch()
.execute(plan)
.await?;
```

Advanced users may depend on internal crates, but normal host applications should not need to.

---

## 19. Testing Strategy

Testing begins before UI work.

### 19.1 Deterministic Offline Tests

The following must have deterministic fixture-based tests:

- Minecraft version JSON parsing;
- version inheritance and merge;
- rule evaluation;
- Maven coordinate parsing;
- library selection;
- native selection;
- argument generation;
- classpath generation;
- Java requirement solving;
- component graph resolution;
- dependency graph behavior;
- pack manifest parsing;
- `InstallPlan`;
- `LaunchPlan`.

### 19.2 Frozen Fixtures

Representative fixtures should cover multiple eras of Minecraft and loader metadata.

Suggested categories:

```text
tests/fixtures/
├── minecraft/
├── fabric/
├── forge/
├── neoforge/
├── modrinth/
├── curseforge/
└── modpacks/
```

### 19.3 Snapshot Tests

Resolved plans should be snapshot-testable.

For example:

```text
fixture metadata
      |
      v
LaunchPlan
      |
      v
stable normalized snapshot
```

### 19.4 Fuzzing

Fuzz targets should prioritize untrusted parsers:

- ZIP/archive manifests;
- NBT;
- Minecraft version JSON;
- mod metadata;
- pack manifests.

---

## 20. Roadmap and Exit Criteria

### Phase 0 — Foundation

> The normative detailed implementation plan for this phase is [`PHASE_0_IMPLEMENTATION_PLAN.md`](PHASE_0_IMPLEMENTATION_PLAN.md).

**Implement**

- workspace skeleton;
- `graphene-core`;
- `graphene-platform`;
- `graphene-network`;
- structured errors;
- operation/event/progress model;
- initial storage layout.

**Exit criteria**

- workspace dependency rules compile cleanly;
- a long-running dummy operation can be cancelled and observed;
- network downloader can fetch, verify, and cache a fixture artifact;
- architecture tests or dependency checks prevent obvious reverse dependencies.

---

### Phase 1 — Vanilla Vertical Slice

**Implement**

- official Minecraft metadata;
- version resolution;
- artifacts;
- `InstallRequest` / `InstallPlan`;
- transactional installation;
- Java discovery;
- Java selection;
- `LaunchRequest` / `LaunchPlan`;
- process execution.

**Exit criteria**

- an empty data directory can become a launchable Vanilla instance;
- dry-run `LaunchPlan` is deterministic;
- reinstall does not redownload valid cached artifacts;
- cancellation leaves no corrupted committed instance;
- launch output can be streamed without UI dependencies.

This is the first major project proof.

---

### Phase 2 — Authentication and Managed Java

**Implement**

- Microsoft;
- offline accounts;
- token refresh;
- secure secret storage;
- Java runtime management/download;
- compatibility diagnostics.

**Exit criteria**

- authenticated and offline sessions can feed the same launch pipeline;
- secrets never appear in exported logs;
- incompatible Java produces structured diagnostics.

---

### Phase 3 — Component/Loader System

**Implement**

- Fabric;
- NeoForge;
- Forge;
- component graph;
- provider registry for loader metadata.

**Exit criteria**

- loader installation does not require loader-specific branches in launch orchestration;
- each supported loader produces a normalized `ResolvedMinecraft`;
- loader changes use the same transaction/install machinery.

---

### Phase 4 — Instance Engine

**Implement**

- multiple instances;
- clone/delete/rename;
- configuration inheritance;
- instance locks;
- lockfile;
- verification and repair.

**Exit criteria**

- instances remain independent;
- concurrent conflicting mutations are prevented;
- repair is produced as a plan derived from lockfile/filesystem differences.

---

### Phase 5 — Content

**Implement**

- local mod inventory;
- normalized content model;
- Modrinth;
- CurseForge;
- dependencies;
- install/update;
- content compatibility checks.

**Exit criteria**

- services operate on Graphene content types rather than provider DTOs;
- content updates route through the artifact/install pipeline;
- provider capability checks replace provider-name branching.

---

### Phase 6 — Modpacks

**Implement**

- `.mrpack`;
- CurseForge packs;
- Prism/MultiMC import;
- generic archive/URL import;
- Graphene pack format.

**Exit criteria**

- all supported formats normalize into one `PackManifest`/install specification;
- no pack format owns a separate installation executor;
- unsafe archive paths are rejected.

---

### Phase 7 — Diagnostics

**Implement**

- Java diagnostics;
- installation verification;
- crash collection;
- log parsing;
- basic mod/loader evidence;
- diagnostic export and redaction.

**Exit criteria**

- hosts can display useful diagnostics without parsing arbitrary log text;
- repair suggestions resolve into explicit plans rather than hidden mutations.

---

### Phase 8 — Host Integrations

**Implement**

- Tauri adapter and/or Slint controller;
- CLI examples/reference client.

**Exit criteria**

- at least two host styles can consume the same backend API without forking launcher logic;
- Graphene crates remain free of UI framework dependencies.

---

## 21. Non-Goals and Explicit Boundaries

The following are outside the Graphene core product unless introduced later through an explicit host
or extension boundary:

- UI layout, widgets, themes, animation, and localization;
- launcher news feeds;
- advertising;
- social/community features;
- store or payment systems;
- first-party server hosting;
- launcher self-update implementation;
- dynamic native Rust plugins;
- arbitrary shell scripting as a default execution model;
- cloud synchronization as a core requirement;
- undocumented manipulation of third-party launchers' private internal state.

Graphene may expose interfaces that hosts can use to implement some of these, but they must not
become dependencies of launcher domain logic.

---

## 22. Plugin and Extension Policy

Version 1 should use statically registered providers:

```text
ProviderRegistry
    .register(...)
```

Graphene should not load arbitrary Rust `.dll`, `.so`, or `.dylib` plugins as its public extension
ABI.

If third-party runtime plugins are required later, preferred directions are:

- subprocess/IPC protocol;
- WASM guest interface;
- another versioned language-neutral protocol.

The reason is long-term ABI stability and fault isolation.

---

## 23. Performance and Cache Direction

A later optimization layer may use a content-addressed store:

```text
cache/objects/
└── sha256/
    └── ab/
        └── abcdef...
```

Potential flow:

```text
source URL
    |
    v
temporary file
    |
    v
hash verification
    |
    v
CAS object
    |
    +--> hardlink when supported
    |
    +--> copy fallback
```

This should remain an implementation detail of the artifact/cache layer and must not leak into
domain APIs.

---

## 24. Architecture Invariants

The following rules should be copied into `CONTRIBUTING.md` and enforced through review and, where
practical, automated dependency checks.

1. Domain crates do not depend on UI frameworks.
2. Domain crates do not depend on concrete providers.
3. Provider DTOs do not cross adapter boundaries.
4. No global mutable singleton is required for normal use.
5. Raw HTTP calls do not appear throughout business logic.
6. Business modules do not implement their own file downloader.
7. Minecraft launch uses argv, not shell-string concatenation.
8. Public behavior does not depend on parsing human-readable error messages.
9. Secrets are never intentionally written to logs.
10. Installation and repair go through `InstallPlan`.
11. Launch execution goes through `LaunchPlan`.
12. Artifacts are verified when trustworthy metadata provides hashes.
13. Long-running operations are cancellable.
14. Long-running operations expose unified progress.
15. Complex instance mutations provide failure recovery.
16. Cache/database data is not the only copy of user-owned instance state.
17. Public APIs do not expose third-party provider DTOs.
18. Public APIs do not expose Tauri or Slint types.
19. New loaders extend component/provider abstractions before changing launch core.
20. New content services extend `ContentProvider` before adding provider-specific service branches.
21. Pack formats normalize into one installation model.
22. Repair reuses installation execution primitives.
23. Architecture changes that violate these rules require an explicit ADR.

---

## 25. Definition of Done for Version 1.0

Version 1.0 is not defined by the existence of every possible loader or catalog integration.

It is defined by architectural completeness and a stable backend contract.

Graphene 1.0 should satisfy all of the following:

- stable root facade suitable for desktop UI and CLI hosts;
- multiple isolated instances;
- Vanilla installation and launch;
- Microsoft and offline accounts;
- automatic Java discovery and compatible runtime selection;
- at least Fabric, NeoForge, and Forge support;
- deterministic `InstallPlan` and `LaunchPlan`;
- transactional install/repair behavior;
- lockfile-based verification;
- Modrinth and CurseForge content integration;
- local mod management and update workflow;
- at least `.mrpack` plus one major external pack format;
- structured progress, cancellation, errors, events, and diagnostics;
- secure secret storage and redacted diagnostic output;
- Windows/Linux/macOS support at the abstraction level, with CI coverage for supported target
  combinations;
- fixture-based tests for Minecraft resolution and launch planning;
- no UI framework dependency in the backend;
- no external provider DTO leakage through the public API;
- documented architecture and dependency boundaries.

Additional loaders, more pack formats, advanced diagnostics, launcher self-update hooks, and
external plugin protocols may be post-1.0 work.

---

## 26. Immediate Next Engineering Actions

Implementation should begin in the following order:

1. Convert the single package into a Cargo workspace while preserving `graphene` as the root facade.
2. Create `graphene-core`, `graphene-platform`, `graphene-network`, `graphene-minecraft`,
   `graphene-instance`, `graphene-install`, `graphene-java`, `graphene-launch`, and
   `graphene-service`.
3. Define IDs, errors, diagnostics, operations, progress, cancellation, and `Artifact`.
4. Define provider-neutral HTTP/download interfaces.
5. Define Minecraft metadata and `ResolvedMinecraft`.
6. Define `InstallRequest`, `InstallPlan`, and transaction interfaces.
7. Define Java runtime discovery and requirement solving.
8. Define `LaunchRequest` and `LaunchPlan`.
9. Build deterministic fixtures and plan snapshot tests.
10. Only after the Vanilla vertical slice succeeds, add authentication and loader providers.

The guiding rule for early development is:

> Every new implementation should strengthen the Vanilla install-to-launch path or establish a
> boundary required by that path.

---

## 27. Decision Summary

The architecture can be summarized in four decisions:

### Decision A — Graphene is an engine, not an application shell

UI frameworks consume Graphene; Graphene never consumes them.

### Decision B — Resolve into stable plans before performing side effects

`InstallPlan` and `LaunchPlan` are central testable products of the engine.

### Decision C — External services are adapters

Mojang, Modrinth, CurseForge, Fabric, Forge, NeoForge, Microsoft authentication, and Java
distributions are replaceable provider implementations around stable domain models.

### Decision D — Extreme modularity means strict boundaries, not excessive crate count

Approximately fifteen bounded-context crates are enough. The critical property is one-way dependency
flow and explicit ownership.

---

## 28. Architectural North Star

The project should continuously preserve this end-state:

```text
                        Graphene Facade
                              |
                        Service Layer
                              |
          +-------------------+-------------------+
          |                   |                   |
       Install              Launch            Diagnostics
          |                   |                   |
          +----------- Stable Domain ------------+
                              |
     +------------+-----------+-----------+-------------+
     |            |           |           |             |
 Minecraft     Instance      Auth        Java         Content
     |            |           |           |             |
     +------------+---- Provider Interfaces ------------+
                              |
                      Concrete Adapters
                              |
     +-------------+----------+----------+---------------+
   Mojang        Fabric      Forge    Modrinth      CurseForge ...
                              |
                     Network / Storage
                              |
                          Platform
```

And these two flows remain the center of the product:

```text
InstallRequest
      |
      v
Resolution
      |
      v
InstallPlan
      |
      v
Artifact Pipeline
      |
      v
Transactional Commit
```

```text
LaunchRequest
      |
      v
Instance + Auth + Java
      |
      v
Minecraft Resolution
      |
      v
LaunchPlan
      |
      v
Process
      |
      v
Events / Logs / Diagnostics
```

If these boundaries remain intact, choosing Tauri, Slint, CLI, or another host remains a
presentation decision rather than an architectural rewrite.
