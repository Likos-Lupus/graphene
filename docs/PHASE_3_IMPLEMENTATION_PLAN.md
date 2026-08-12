# Graphene Phase 3 — Detailed Loader Component System Implementation Plan

**Status:** Approved implementation plan  
**Phase:** 3 — Loader Components  
**Primary implementation language:** Rust 2024  
**Implementation state:** Phase 3 implementation candidate present in this worktree; automated Cargo
validation and real loader smoke sign-off remain pending in the current execution environment  
**Planning baseline:** 2026-08-12  
**Normative parent documents:** `PROJECT_SPECIFICATION.md`, `ARCHITECTURE.md`,
`SCOPE_AND_BOUNDARIES.md`, `TARGET_DELIVERABLE.md`, `ROADMAP.md`, all Phase 0 / Phase 1 / Phase 2
implementation, API, security, and smoke-test documents, and all accepted ADRs already present in
the repository.

---

## 1. Purpose

Phase 3 transforms Graphene from a Vanilla-only Minecraft resolver into a **component-composed
Minecraft runtime engine** without weakening the boundaries established by Phases 0–2.

The phase introduces:

- a provider-neutral component graph;
- exact loader-version selection;
- deterministic component ordering;
- normalized Minecraft metadata patches;
- a loader provider registry;
- Fabric Loader support;
- Forge support;
- NeoForge support;
- normalized Forge-family installer recipes;
- bounded processor execution in staging;
- generated-output verification and publication;
- persisted exact component identity;
- loader-aware support reporting, errors, and diagnostics.

The architectural center is:

```text
Minecraft base metadata
        |
        v
Base Minecraft Component
        |
        +-----------------------------+
        |                             |
        v                             v
 Loader Component(s)           requirements/conflicts
        |                             |
        +--------------+--------------+
                       |
                       v
                 Component Graph
                       |
                       v
              MinecraftVersionPatch
                       |
                       v
                ResolvedMinecraft
                       |
             +---------+----------+
             |                    |
             v                    v
        InstallPlan          launch metadata
             |
             v
    transactional preparation
             |
             v
        InstallReceipt
             |
             v
      existing LaunchPlan
```

A loader is **not** a special launch mode.

A loader is a component/provider contribution that must converge into the same
`ResolvedMinecraft`, `InstallPlan`, `InstallReceipt`, `JavaRuntime`, `LaunchSession`, and
`LaunchPlan` contracts already used by Vanilla.

---

## 2. Phase 3 Objective

The primary engineering objective is:

> Graphene can resolve an exact Fabric, Forge, or NeoForge loader release for a supported Minecraft
> base version; validate a deterministic component graph; normalize provider metadata into a
> Graphene-owned Minecraft patch and, where required, an explicit installation-preparation recipe;
> enumerate all remote and generated inputs before committed instance mutation; execute required
> preparation inside bounded staging; verify generated outputs; transactionally publish a
> loader-backed instance; persist exact component identity and complete final launch metadata; and
> launch the committed instance through the unchanged provider-neutral Phase 1 launch pipeline.

Phase 3 must establish durable contracts for concepts equivalent to:

```text
ComponentUid
ComponentVersion
ComponentKind
ComponentDescriptor
ComponentRequest
ComponentRequirement
ComponentConflict
ComponentGraph
ResolvedComponent
MinecraftVersionPatch

LoaderKind
LoaderVersion
LoaderVersionSummary
LoaderVersionSelector
LoaderSelection
LoaderSupport
LoaderProviderCapabilities

ComponentPreparationRecipe
InstallerEmbeddedInput
ProcessorDataValue
InstallProcessor
ProcessorArgument
ProcessorOutput
GeneratedArtifactDescriptor
```

Exact Rust names may change during implementation if they remain faithful to these responsibilities.

---

## 3. Prior-Phase Gate

Phase 3 implementation must preserve every Phase 0–2 invariant.

Before Phase 3 can be marked complete, executable evidence must exist for all supported quality
gates:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
python scripts/check_architecture.py
```

Real smoke-test records inherited from earlier phases must remain honest.

At the time this plan is introduced, the repository documentation still treats Phase 2 as an
implementation candidate and records real-smoke prerequisites that have not all been signed off.
This plan does not rewrite those facts.

Rules:

- do not fabricate prior smoke results;
- do not mark unchecked prior-phase gates as passed without evidence;
- do not hide a prior regression to make Phase 3 appear green;
- Phase 3 may be developed as an implementation candidate when an external smoke prerequisite is
  unavailable, but it must not be declared final `DONE`.

---

## 4. Phase 3 Architectural Invariants

Phase 3 must preserve these rules:

1. domain crates do not depend on UI frameworks;
2. domain crates do not depend on concrete loader providers;
3. provider DTOs do not cross adapter boundaries;
4. raw provider HTTP does not spread through Minecraft/install/launch logic;
5. no second downloader is introduced;
6. all remote artifacts use the existing `Artifact` acquisition path;
7. complex mutation is planned before execution;
8. instance mutation remains transactional;
9. long-running work remains cancellable and observable;
10. launch planning remains offline after committed state exists;
11. `graphene-launch` remains unaware of Fabric, Forge, and NeoForge;
12. loader-specific provider code remains in `graphene-providers`;
13. component graph and metadata composition remain in stable domain code;
14. installer execution remains in `graphene-install`;
15. Java selection remains provider-neutral;
16. authentication remains orthogonal to loader resolution;
17. committed user-owned state remains recoverable from files;
18. public behavior remains machine-readable through structured errors/diagnostics;
19. crate roots remain thin facades;
20. Phase 4 must be able to reuse Phase 3 primitives for future loader changes.

The most important Phase 3 rule is:

> **Loaders may change resolution and installation preparation, but they must not become branches in
> the launch engine.**

---

## 5. Scope

### 5.1 In Scope

Phase 3 includes:

- component identity and version identity;
- component requirements and conflicts;
- deterministic graph construction;
- cycle detection;
- deterministic graph ordering;
- normalized Minecraft metadata patches;
- final component-composed `ResolvedMinecraft`;
- exact loader version queries and selection;
- loader capability/support reporting;
- provider registry;
- Fabric loader provider;
- Forge provider;
- NeoForge provider;
- Fabric profile normalization;
- Forge legacy installer/profile support where explicitly declared;
- Forge modern installer/profile support;
- NeoForge current supported installer/profile support;
- provider-specific version discovery and normalization;
- verified installer artifact acquisition;
- bounded installer JAR parsing;
- normalized preparation recipes;
- embedded installer inputs;
- processor data variables;
- processor placeholder expansion;
- processor Java requirements;
- generic installation-tool runner;
- client-side processor filtering;
- processor timeout and cancellation;
- bounded stdout/stderr handling;
- generated output validation;
- generated output provenance;
- safe generated output publication;
- generated output reuse;
- component-aware installation request/planning;
- install-plan schema/version evolution;
- install-receipt schema/version evolution;
- Phase 1 Vanilla receipt compatibility;
- loader-aware errors and diagnostics;
- deterministic fixtures;
- hostile fixtures;
- snapshot/property/fuzz tests;
- architecture checks;
- cross-platform CI;
- real smoke procedures and support matrix.

### 5.2 Explicitly Out of Scope

Phase 3 must not implement:

- Fabric API as an implicit loader dependency;
- arbitrary mods;
- Modrinth;
- CurseForge;
- content dependency resolution;
- modpacks;
- Quilt;
- Legacy Fabric unless explicitly added as a separate future provider;
- LiteLoader;
- OptiFine;
- Cleanroom;
- server installation;
- server run-script generation;
- arbitrary shell scripts;
- arbitrary provider-supplied executables;
- a general JVM sandbox;
- third-party authentication;
- a new Java distribution subsystem;
- Phase 4 instance mutation/repair features;
- in-place loader upgrade/downgrade as a special provider operation;
- a loader-specific downloader;
- dynamic native plugins.

---

## 6. Upstream Adapter Basis

This section records the intended adapter direction. It is not a permanent promise that upstream
APIs will never change.

Implementation must verify current contracts against **official primary sources** when Phase 3 code
is written.

### 6.1 Fabric

The intended Fabric path is:

```text
official Fabric Meta
      |
      v
exact Minecraft + loader pair
      |
      v
exact profile JSON
      |
      v
normalized MinecraftVersionPatch
```

Fabric should prove that a loader can be installed by component composition without introducing an
installer-process framework.

### 6.2 Forge

Forge support must treat the installer as a verified source of:

```text
launcher metadata
data variables
processor declarations
processor dependencies
generated output expectations
```

Graphene must normalize those semantics instead of delegating committed-state mutation to an opaque
installer process.

### 6.3 NeoForge

NeoForge must use the same stable output model as Forge where installer semantics genuinely match,
while remaining a distinct provider adapter with provider-specific version discovery and DTOs.

### 6.4 Protocol Volatility

The stable domain must model:

```text
component identity
component compatibility
metadata patch
verified installer input
processor recipe
generated output
```

It must not expose one current upstream JSON/XML response shape as Graphene's permanent public API.

---

## 7. Product-Level Flows

### 7.1 Vanilla

```text
MinecraftVersionId
      |
      v
Mojang provider
      |
      v
Minecraft base component
      |
      v
ComponentGraph
      |
      v
ResolvedMinecraft
      |
      v
existing InstallPlan / execution
```

Vanilla is the base component.

No fake "Vanilla loader provider" is required.

### 7.2 Fabric

```text
Minecraft base
      |
      +--> exact Fabric selection
                 |
                 v
           Fabric provider
                 |
                 v
      normalized loader component
                 |
                 v
       MinecraftVersionPatch
                 |
                 v
          ComponentGraph
                 |
                 v
        ResolvedMinecraft
                 |
                 v
            InstallPlan
```

### 7.3 Forge / NeoForge

```text
Minecraft base
      |
      +--> exact loader selection
                 |
                 v
           loader provider
                 |
           +-----+------+
           |            |
           v            v
     metadata patch   preparation recipe
           |            |
           +------+-----+
                  |
                  v
           ComponentGraph
                  |
                  v
          ResolvedMinecraft
                  |
                  v
            InstallPlan v2
                  |
       +----------+-----------+
       |                      |
       v                      v
verified remote inputs   processor steps
       |                      |
       +----------+-----------+
                  |
                  v
             staging root
                  |
                  v
       verify generated outputs
                  |
                  v
        transactional publication
```

---

## 8. Loader Support Definition

Graphene must not claim broad loader support because one version happens to parse.

A loader release is fully supported only when:

1. its base Minecraft version is supported for installation;
2. the exact loader release can be resolved;
3. loader metadata is normalized successfully;
4. required remote artifacts meet integrity policy;
5. the installer/profile schema family is supported;
6. all required client-side preparation steps are understood;
7. required Java can be selected or explicitly ensured;
8. generated outputs can be validated;
9. a deterministic `InstallPlan` can be built;
10. a committed receipt can rebuild an offline `LaunchPlan`.

---

## 9. Loader Support Tiers

### 9.1 Tier A — Install and Launch Supported

Full path:

```text
query
 -> exact resolve
 -> component graph
 -> patch composition
 -> complete plan
 -> staged preparation
 -> verified commit
 -> offline launch plan
```

### 9.2 Tier B — Metadata/Resolution Supported

The release can be identified and partially normalized, but full safe install is not supported.

Reasons may include:

- unsupported installer schema;
- unsupported processor semantic;
- unsupported placeholder;
- unacceptable artifact integrity;
- unsupported base Minecraft release;
- unavailable installer Java;
- incomplete provider metadata.

Tier B must be explicit and machine-readable.

### 9.3 Unsupported

Malformed, unknown, or unsafe formats fail structurally.

There is no fallback equivalent to:

```text
run the installer and hope it works
```

---

## 10. Minimum Format Families for Sign-Off

The Phase 3 support matrix must contain pinned exact fixture coverage for at least:

```text
Fabric profile-based loader
Forge modern processor-profile family
NeoForge modern processor-profile family
```

If legacy Forge is declared Tier A, it must have a separately pinned legacy fixture family.

Moving "latest" releases must not be used as deterministic test identity.

---

## 11. Repository / Bounded-Context Strategy

No new top-level loader crate is required by default.

Existing ownership already maps cleanly:

```text
graphene-minecraft
    component graph
    patch semantics
    normalized resolved Minecraft

graphene-providers
    Fabric / Forge / NeoForge adapters
    provider DTOs
    loader registry/adapters

graphene-install
    preparation execution
    processor runner
    generated output publication

graphene-instance
    persisted component identity / receipt schema

graphene-java
    Java requirement compatibility

graphene-service
    orchestration and dependency wiring

graphene-launch
    unchanged loader-neutral launch planning
```

A new crate should be introduced only if implementation evidence reveals a genuine new bounded
context.

---

## 12. Thin `lib.rs` Rule

Phase 3 must not recreate monolithic crate roots.

A crate-root `lib.rs` may contain:

- crate docs;
- module declarations;
- deliberate re-exports;
- tiny crate-wide constants.

It must not contain substantial:

- graph algorithms;
- patch merge algorithms;
- Fabric DTO parsing;
- Forge installer parsing;
- NeoForge installer parsing;
- Maven XML logic;
- processor planning;
- placeholder expansion;
- processor loops;
- generated output publication.

Architecture tooling should guard obvious regressions.

---

## 13. Proposed `graphene-minecraft` Layout

Recommended direction:

```text
crates/graphene-minecraft/src/
├── lib.rs
├── existing Phase 1 modules...
├── component/
│   ├── mod.rs
│   ├── uid.rs
│   ├── version.rs
│   ├── kind.rs
│   ├── descriptor.rs
│   ├── request.rs
│   ├── requirement.rs
│   ├── conflict.rs
│   ├── graph.rs
│   ├── resolve.rs
│   └── support.rs
├── patch/
│   ├── mod.rs
│   ├── model.rs
│   ├── merge.rs
│   └── validation.rs
└── loader/
    ├── mod.rs
    ├── kind.rs
    ├── selection.rs
    ├── release.rs
    ├── capability.rs
    └── preparation/
        ├── mod.rs
        ├── recipe.rs
        ├── embedded.rs
        ├── value.rs
        ├── processor.rs
        └── output.rs
```

Exact filenames may evolve while preserving ownership.

---

## 14. Proposed `graphene-providers` Layout

Recommended direction:

```text
crates/graphene-providers/src/
├── lib.rs
├── existing providers...
└── loader/
    ├── mod.rs
    ├── provider.rs
    ├── registry.rs
    ├── common/
    │   ├── mod.rs
    │   ├── maven.rs
    │   ├── checksum.rs
    │   └── installer_profile.rs
    ├── fabric/
    │   ├── mod.rs
    │   ├── config.rs
    │   ├── dto.rs
    │   ├── normalize.rs
    │   └── provider.rs
    ├── forge/
    │   ├── mod.rs
    │   ├── config.rs
    │   ├── dto/
    │   ├── discovery.rs
    │   ├── installer.rs
    │   ├── normalize.rs
    │   └── provider.rs
    └── neoforge/
        ├── mod.rs
        ├── config.rs
        ├── dto/
        ├── discovery.rs
        ├── installer.rs
        ├── normalize.rs
        └── provider.rs
```

Shared Forge/NeoForge code is allowed only for genuinely shared format semantics.

---

## 15. Proposed `graphene-install` Layout

Recommended additions:

```text
crates/graphene-install/src/
├── lib.rs
├── existing modules...
├── component/
│   ├── mod.rs
│   ├── plan.rs
│   └── materialize.rs
├── processor/
│   ├── mod.rs
│   ├── runner.rs
│   ├── expand.rs
│   ├── validate.rs
│   ├── output.rs
│   └── execution.rs
├── generated/
│   ├── mod.rs
│   ├── descriptor.rs
│   ├── verify.rs
│   └── publish.rs
└── staging/
    └── loader.rs
```

Existing Phase 1 executor modules should be extended, not replaced.

---

## 16. Proposed `graphene-service` Layout

Recommended additions:

```text
crates/graphene-service/src/
├── lib.rs
├── existing modules...
├── loader_service/
│   ├── mod.rs
│   ├── versions.rs
│   ├── resolve.rs
│   └── support.rs
├── component_service/
│   ├── mod.rs
│   └── compose.rs
└── install_service/
    ├── mod.rs
    ├── plan.rs
    └── component_plan.rs
```

If an existing flat service file becomes large because of Phase 3, split it before it becomes a god
file.

---

## 17. Crate Responsibilities

### `graphene-core`

May add only stable generic IDs/error codes/diagnostic primitives where necessary.

Must not own loader metadata or algorithms.

### `graphene-minecraft`

Owns:

- component identities;
- graph semantics;
- requirements/conflicts;
- normalized loader release models;
- patch model and merge rules;
- final composed `ResolvedMinecraft`;
- provider-neutral preparation descriptions.

Must not own HTTP/provider DTOs/process execution.

### `graphene-providers`

Owns:

- Fabric adapter;
- Forge adapter;
- NeoForge adapter;
- provider-specific version semantics;
- Maven/provider DTOs;
- conversion to stable Graphene models.

Must not mutate instances.

### `graphene-install`

Owns:

- install-plan evolution;
- preparation execution;
- tool-runner port;
- processor argument expansion;
- staged execution;
- output validation/publication;
- cancellation/cleanup.

Must not parse provider DTOs.

### `graphene-java`

Owns normalized Java requirements and compatibility.

It must not know Forge/NeoForge installer schema.

### `graphene-instance`

Owns persisted component identity and receipt schema.

### `graphene-launch`

Remains loader-neutral.

### `graphene-service`

Wires provider registry, Java/tool runner, planning, and services.

It must not reimplement parsing/graph logic.

### Root `graphene`

Exports curated stable types/services only.

---

## 18. Required Dependency Direction

Conceptually:

```text
                             graphene
                                |
                                v
                        graphene-service
              +---------+------+------+---------+
              |                |      |         |
              v                v      v         v
        graphene-install  providers  java    instance
              |                |      |         |
              |                v      |         |
              +--------> graphene-minecraft <---+
                              |
                              v
                        graphene-core
```

Allowed adapter edges include:

```text
graphene-providers -> graphene-network
graphene-providers -> graphene-minecraft
graphene-providers -> graphene-core
```

Forbidden:

```text
graphene-minecraft -> graphene-providers
graphene-minecraft -> graphene-network
graphene-minecraft -> graphene-service
graphene-minecraft -> graphene-install

graphene-install -> graphene-providers
graphene-install -> graphene-network

graphene-launch -> graphene-providers
graphene-launch -> Fabric/Forge/NeoForge modules

graphene-java -> graphene-providers

graphene-service -> private provider DTO modules

any backend crate -> tauri
any backend crate -> slint
```

If processor execution requires Java selection, dependency inversion is required rather than a
cycle.

---

## 19. `ComponentUid`

Introduce a Graphene-owned opaque component UID.

Examples:

```text
net.minecraft
net.fabricmc.fabric-loader
net.minecraftforge.forge
net.neoforged.neoforge
```

Requirements:

- non-empty;
- bounded length;
- normalized safe character set;
- no path separators;
- no control characters;
- stable serialization;
- provider IDs normalized at adapter boundaries.

The UID identifies a logical component family, not one release.

---

## 20. `ComponentVersion`

Component versions are opaque validated strings.

Do not impose global SemVer ordering.

Different loader families have different version schemes.

The stable domain needs:

```text
identity
equality
serialization
bounded validation
display
```

Ordering/recommendation logic remains provider-specific.

---

## 21. `ComponentKind`

Recommended non-exhaustive variants:

```text
Minecraft
Loader
Auxiliary
```

Phase 3 permits:

- exactly one Minecraft base;
- zero or one primary loader.

`Auxiliary` reserves room for future componentized runtime metadata without prematurely modeling
mods/content as loader components.

---

## 22. `ComponentDescriptor`

Conceptually:

```rust
pub struct ComponentDescriptor {
    pub uid: ComponentUid,
    pub version: ComponentVersion,
    pub kind: ComponentKind,
    pub order: i32,
    pub requires: Vec<ComponentRequirement>,
    pub conflicts: Vec<ComponentConflict>,
}
```

`order` is a deterministic patch-order hint, not a way to bypass dependencies.

---

## 23. Component Requests

A component request expresses desired logical state before exact provider resolution.

Minimum semantics:

```text
component family
version selector
optional policy
```

For loader convenience, a public `LoaderSelection` may be used.

Every executable install plan must contain exact resolved versions.

---

## 24. Component Requirements

Keep the Phase 3 solver intentionally small.

Required semantics:

```text
exact component presence
exact Minecraft base version
```

Initial requirement forms may be:

```text
Exact
Any
```

Do not build a universal package manager.

Provider-owned selectors resolve recommendations/ranges before graph construction.

---

## 25. Component Conflicts

Fabric, Forge, and NeoForge are primary loaders and conflict.

The graph must reject:

```text
Fabric + Forge
Fabric + NeoForge
Forge + NeoForge
```

structurally.

Do not rely on scattered service-side string comparisons.

---

## 26. Graph Invariants

A valid Phase 3 component graph has:

- exactly one Minecraft base component;
- zero or one primary loader;
- unique UIDs;
- satisfied requirements;
- no conflicts;
- no cycles;
- deterministic topological order;
- bounded node count;
- bounded edge count.

---

## 27. Deterministic Ordering

Topological ordering must be stable.

When multiple nodes are eligible, use Graphene-owned tie-breakers such as:

```text
order
then component UID
then exact component version
```

Never rely on hash-map iteration order.

---

## 28. Cycle Detection

Cycles produce structured errors before mutation.

The diagnostic includes safe normalized component identities.

Graph processing must have explicit node/edge bounds.

---

## 29. Graph Resolution Algorithm

Recommended order:

1. validate base Minecraft request;
2. resolve exact base Minecraft metadata;
3. resolve exact requested loader release;
4. construct component nodes;
5. validate unique identities;
6. validate requirements;
7. validate conflicts;
8. detect cycles;
9. topologically order;
10. compose patches;
11. validate final metadata;
12. collect preparation recipes;
13. resolve every remote artifact;
14. build final `ResolvedMinecraft`;
15. build component-aware `InstallPlan`.

All provider/network-dependent selection completes before execution.

---

## 30. `MinecraftVersionPatch`

Introduce a Graphene-owned partial metadata patch.

Potential contributions:

```text
main class override
libraries
JVM arguments
game arguments
Java requirement contribution
logging contribution when valid
component provenance
```

A patch must not contain provider DTOs, callbacks, or raw installer handles.

---

## 31. Patch Composition

Composition is ordered:

```text
base metadata
   |
component patch 1
   |
component patch 2
   |
final normalized metadata
```

Reuse/refactor Phase 1 inheritance/merge semantics where compatible.

Do not implement a second incompatible library merge model.

---

## 32. Scalar Patch Semantics

For scalar fields:

```text
patch absent  -> preserve
patch present -> replace
```

Final required fields must validate.

A loader cannot erase the runtime into an invalid state.

---

## 33. Library Patch Semantics

Libraries merge by normalized Maven identity.

Required:

- deterministic order;
- component replacement of the same identity where upstream semantics require it;
- no duplicate classpath destination;
- Phase 1 OS/rule filtering remains effective;
- loader repository URLs become normalized artifact sources.

Do not compare raw provider JSON.

---

## 34. Argument Patch Semantics

JVM and game arguments remain structured `Argument` values.

Default semantics:

```text
append in deterministic component order
```

unless a supported upstream schema requires an explicitly modeled replacement semantic.

Never flatten arguments into shell command text.

---

## 35. Base Minecraft Identity

Keep the base Minecraft version distinct from loader identity.

Prefer keeping:

```text
ResolvedMinecraft.version_id = base Minecraft version
```

and adding an explicit component set.

Do not encode loader state in synthetic values such as:

```text
1.21.1-forge-...
fabric-loader-...-1.21.1
```

Later phases must not need to parse synthetic strings to discover loader state.

---

## 36. Resolved Component Set

`ResolvedMinecraft` should gain an exact deterministic component set or equivalent.

Conceptual:

```text
ResolvedComponent
├── uid
├── exact version
├── kind
└── safe provenance
```

Vanilla:

```text
net.minecraft <base>
```

Fabric:

```text
net.minecraft <base>
net.fabricmc.fabric-loader <exact>
```

Forge/NeoForge follow the same model.

---

## 37. Game Java Requirement Merge

Loader patches may contribute a game Java requirement.

Baseline merge:

```text
same requirement        -> valid
component absent        -> preserve base
base absent             -> accept component
incompatible conflict   -> structured failure
```

If real upstream fixtures require richer Java ranges, evolve the Java model explicitly with an ADR
rather than embedding loader-specific conditions.

---

## 38. Installer Java vs Game Java

Forge-family processors may need a Java runtime different from the game runtime.

Model them separately.

Expected:

```text
game Java requirement
     -> final ResolvedMinecraft
     -> normal launch selection

installer-tool Java requirement
     -> preparation processor
     -> Phase 2 Java selection/ensure
```

A loader provider must never call a concrete Java distribution provider.

---

## 39. Loader Family

Expose a normalized non-exhaustive family such as:

```text
Fabric
Forge
NeoForge
```

Future variants may add Quilt or other loaders without altering launch.

---

## 40. Loader Version Selection

Executable plans require exact loader versions.

Public request selectors may include:

```text
Exact(version)
LatestStable
Recommended
```

but dynamic selectors must resolve before the plan is returned.

The plan and receipt record exact versions.

Fixtures/snapshots always use exact versions.

---

## 41. Loader Version Summary

Normalized query output may contain:

```text
loader family
exact version
compatible Minecraft base
stability/channel
provider-authoritative recommended flag if available
release timestamp if available
support classification
```

Do not fabricate provider concepts.

---

## 42. Loader Provider Interface

A provider abstraction should support concepts equivalent to:

```text
list versions for base Minecraft
resolve exact loader release
normalize component metadata
normalize preparation recipe
report support
```

Returned values are Graphene domain types.

---

## 43. Loader Provider Registry

Create a registry that maps loader family/capability to provider implementation.

Conceptual:

```text
LoaderProviderRegistry
├── Fabric
├── Forge
└── NeoForge
```

A small construction-time mapping is acceptable.

Use-case logic must not spread:

```rust
match loader_kind { ... }
```

through services and installers.

---

## 44. Provider Capabilities

Potential capabilities:

```text
VERSION_LIST
EXACT_RESOLUTION
PROFILE_PATCH
INSTALLER_ARCHIVE
PROCESSOR_RECIPE
LEGACY_PROFILE
RECOMMENDED_RELEASE
CHECKSUM_SIDECARS
```

Capabilities enable support reporting without exposing DTOs.

---

## 45. Provider Configuration

Each provider gets narrow validated configuration.

Production:

- official HTTPS origins;
- bounded metadata limits;
- safe redirects;
- provider-specific Maven/profile origins.

Fixtures:

- explicit local origin;
- HTTP only in test/fixture mode;
- no global mutable endpoint override.

Literal URLs must not be scattered through services.

---

# Fabric Vertical Slice

## 46. Fabric Objective

Fabric is the simplest proof that components compose into Minecraft metadata.

The Fabric provider should:

1. query versions for a base Minecraft version;
2. resolve an exact loader release;
3. obtain the exact loader profile;
4. normalize main class/libraries/arguments;
5. create a loader component;
6. return a `MinecraftVersionPatch`;
7. declare remote artifacts;
8. normally return no processor recipe.

---

## 47. Fabric Meta Boundary

At implementation time, verify current official Fabric Meta contracts.

The adapter should use exact game+loader profile data where available.

DTOs remain private.

Graphene exposes normalized release/profile values, not Fabric Meta JSON.

---

## 48. Fabric Version Semantics

Provider-specific stability/order data remains inside the adapter.

Do not use global semantic-version assumptions.

Return exact ordered normalized releases.

---

## 49. Fabric Profile Normalization

Normalize:

- main class;
- loader libraries;
- arguments;
- inherited/base relationship;
- safe provenance.

Base assets/client/logging remain supplied by the Minecraft base unless the supported profile
explicitly overrides a field through a documented normalized rule.

---

## 50. Fabric Runtime Libraries

Intermediary/loader libraries become ordinary normalized libraries after provider conversion.

They do not need to become user-visible graph components.

---

## 51. Fabric API Boundary

**Fabric API is not Fabric Loader.**

Phase 3 must not silently install Fabric API.

Fabric API belongs to future content/mod dependency management.

---

## 52. Fabric Installation

Normal Fabric installation should be:

```text
base Minecraft artifacts
+
Fabric loader/profile libraries
+
final normalized receipt
```

No fake Java installer process is introduced.

---

## 53. Fabric Integrity

Use trustworthy upstream integrity where available.

Legacy SHA-1 may remain valid where the ecosystem supplies it.

A locally computed digest may be used for cache/provenance but must not be mislabeled as an
upstream-authenticated checksum.

---

# Forge Vertical Slice

## 54. Forge Objective

Forge must normalize:

1. launcher metadata contribution;
2. installation preparation required to create generated Forge artifacts.

Do not invoke the official installer against the committed Graphene instance directory.

---

## 55. Forge Release Discovery

Provider-specific discovery may use official Maven/file metadata.

Requirements:

- prefer machine-readable official metadata;
- bounded parsing;
- exact version result;
- base Minecraft compatibility validation;
- do not trust a string prefix when authoritative installer metadata contradicts it.

---

## 56. Forge Release Identity

Keep separate:

```text
base Minecraft version
exact Forge version
provider/Maven artifact identity
```

Do not collapse them into one synthetic Graphene version ID.

---

## 57. Forge Installer Acquisition

Sequence:

```text
exact release
   |
installer artifact + trusted integrity
   |
existing Artifact pipeline
   |
verified cache object
   |
bounded installer parse
```

Do not parse executable installer bytes before normal verification policy is satisfied.

---

## 58. Forge Installer Schema Families

Identify supported families explicitly:

```text
legacy launcher-profile family
modern processor-profile family
unknown/unsupported
```

Malformed modern data must not silently fall back to legacy parsing.

---

## 59. Legacy Forge Support

If legacy Forge is Tier A, support exactly the fixture-proven format family.

Possible normalized concerns:

- base Minecraft ID;
- legacy metadata;
- libraries;
- tweak-class/game args;
- universal/loader artifact materialization.

Do not claim universal historical Forge support.

---

## 60. Modern Forge Processor Profiles

Normalize supported modern fields into Graphene models:

```text
profile/spec family
base Minecraft
version metadata
data values
processor declarations
embedded inputs
declared outputs
```

Raw installer DTOs remain private.

---

# NeoForge Vertical Slice

## 61. NeoForge Objective

NeoForge produces the same stable output categories:

```text
exact loader component
metadata patch
verified installer inputs
preparation recipe
```

while remaining a distinct adapter.

---

## 62. NeoForge Version Discovery

Version conventions are provider-specific and may evolve.

The adapter may prefilter using current official rules, but authoritative compatibility should be
validated from release/installer metadata where possible.

The stable domain never assumes one permanent NeoForge version-string grammar.

---

## 63. NeoForge Integrity

Use official Maven checksum sidecars where available.

Prefer the strongest trustworthy supplied digest.

Normalize it into the existing artifact-integrity model.

---

## 64. NeoForge Installer Profile

Where semantics match Forge-family processors, normalize them into the same
`ComponentPreparationRecipe`.

Provider-specific DTO differences remain inside the NeoForge adapter.

---

## 65. Shared Forge-Family Preparation Model

The stable model describes execution semantics rather than brand.

Conceptual:

```text
ComponentPreparationRecipe
├── verified installer artifact
├── embedded inputs
├── data variables
├── processor dependencies
├── processor steps
├── generated outputs
└── installer Java requirements
```

Fabric typically has an empty preparation recipe.

---

# Installer Parsing and Processor Model

## 66. Bounded Installer JAR Parsing

Installer JARs are untrusted executable archives.

Parsing requirements:

- bound archive size;
- bound entry count;
- bound metadata entry size;
- bound embedded input size;
- no arbitrary extraction;
- exact named-entry reads;
- no path traversal;
- no symlink materialization;
- bounded JSON/XML parsing.

Only required entries are inspected.

---

## 67. Installer Metadata Entries

Provider adapters read only supported entries required by their schema family, such as:

```text
install_profile.json
version.json
explicit referenced embedded data
```

Unknown extras are ignored unless resource limits are violated.

---

## 68. Preparation Data Values

Normalize raw installer data into typed values.

Potential categories:

```text
literal
Maven coordinate
embedded installer entry
managed path reference
side-specific value
```

Do not carry provider brace syntax throughout the installer.

---

## 69. Placeholder Allowlist

Processor placeholder expansion is explicit.

Supported normalized placeholders may include equivalents of:

```text
ROOT
INSTALLER
LIBRARY_DIR
MINECRAFT_JAR
MINECRAFT_VERSION
SIDE
MAVEN_PATH
provider data variables
```

The exact allowlist is fixture-driven.

Unknown placeholders are errors.

No arbitrary environment expansion.

---

## 70. Placeholder Path Safety

Path-valued placeholders may resolve only into:

- operation staging;
- verified copied inputs;
- declared shared publication targets;
- declared instance-staging paths.

Provider input must not resolve to arbitrary host filesystem paths.

---

## 71. Client-Side Processor Filtering

Phase 3 installs clients only.

For side constraints:

```text
no side restriction -> execute
contains client      -> execute
server only          -> skip
```

`SIDE` resolves to `client`.

Server run-script generation is out of scope.

---

## 72. Embedded Installer Inputs

Represent required embedded inputs explicitly:

```text
installer entry
staging destination
size limit
optional declared integrity
consumers
```

Extract only from a verified installer.

Compute local digest for deterministic provenance even when the embedded input has no separately
declared upstream checksum.

---

## 73. Processor Model

Conceptual normalized processor:

```text
processor ID
installer-tool Java requirement
executable JAR artifact
classpath artifacts
argv arguments
side condition
declared outputs
```

Every dependency is explicit before execution.

---

## 74. Processor Dependency Acquisition

All processor JAR/classpath inputs must:

- have normalized identity;
- have artifact sources;
- pass the existing acquisition pipeline;
- exist before the step runs.

Processors must not dynamically ask the install crate to download undeclared dependencies.

---

## 75. Processor Entry Point

If the upstream format uses executable JAR semantics, resolve the main class through a
deterministic, bounded manifest mechanism.

Reject missing/ambiguous entry points.

Do not invoke a shell.

---

## 76. Generic Install-Tool Runner Port

`graphene-install` should define a generic runner port.

Responsibilities:

```text
receive explicit Java executable
receive argv/classpath
set bounded working directory
spawn directly
drain stdout/stderr
honor timeout
honor cancellation
return structured result
```

A service/platform adapter wires actual Java selection and process execution.

This avoids `graphene-install -> graphene-service` or `graphene-install -> concrete Java provider`.

---

## 77. Processor Java Selection

Do not use ambient `java` from `PATH` when a specific requirement is known.

Preferred:

```text
processor Java requirement
   |
Phase 2 Java selection/ensure
   |
explicit JavaRuntime
   |
tool runner
```

Whether managed Java download is permitted must be an explicit install policy.

---

## 78. No Shell

Processor execution uses direct argv.

Forbidden:

```text
sh -c
cmd.exe /C
powershell -Command
constructed command strings
```

Provider strings do not become shell scripts.

---

## 79. Minimal Processor Environment

Do not intentionally expose authentication credentials to processor environment variables.

Loader processors should not need:

- Microsoft tokens;
- refresh tokens;
- XSTS tokens;
- Minecraft access tokens.

---

## 80. Synthetic Processor Root

Processors execute against staging:

```text
operation staging/
├── root/
│   ├── libraries/
│   ├── versions/
│   └── ...
├── installer/
├── inputs/
└── outputs/
```

`ROOT` must not point to the committed Graphene data root.

---

## 81. Immutable Cache Isolation

Processor code may modify inputs.

Never expose shared immutable cache objects as writable hardlinks.

Use safe copies or copy-on-write semantics that preserve cache immutability.

---

## 82. Processor Network Policy

Graphene keeps network acquisition centralized.

Where an upstream processor step is semantically just retrieval of a known immutable input and
Graphene can derive the same artifact safely, provider normalization should prefer:

```text
known remote input
 -> Graphene Artifact
 -> verified acquisition
 -> materialize into staging
```

Do this only with fixture-proven equivalence.

Unknown uncontrolled processor-network semantics may be classified unsupported.

---

## 83. Network-Like Processor Normalization

Do not blindly pattern-match arbitrary processor command strings.

Provider adapters may recognize a known official processor semantic only when:

- schema family is supported;
- expected args are exact;
- equivalent upstream artifact metadata is known;
- fixtures prove equivalence.

Otherwise execute the supported verified processor or reject it.

---

## 84. Processor Trust Boundary

Verified processor JARs are executable code.

Graphene Phase 3 does not provide an OS/JVM sandbox.

The security claim is limited to:

```text
trusted configured provider
+
verified processor artifact
+
isolated managed staging
+
direct argv
+
bounded process/output/time
+
validated publication
```

The security review must state this limitation explicitly.

---

## 85. Processor Timeout

Each processor gets a bounded timeout.

On timeout:

1. request termination;
2. enforce bounded kill;
3. drain/close streams;
4. report structured failure;
5. clean staging.

---

## 86. Processor Output Capture

Requirements:

- always drain stdout/stderr;
- bounded in-memory buffers;
- truncation marker;
- invalid UTF-8 handled safely;
- no deadlock on pipe saturation;
- no unbounded raw output copied into errors.

---

## 87. Declared Processor Outputs

Every expected generated output is explicit.

Model:

```text
staging output
managed final destination
expected digest if declared
expected size if declared
producer processor
publication scope
```

Files not declared by the plan are not automatically published.

---

## 88. Output Verification

When upstream declares integrity:

```text
generated file
 -> hash
 -> compare
 -> publish only on match
```

For deterministic locally derived outputs without upstream output integrity:

- verify existence/type/size bounds;
- compute local SHA-256;
- record provenance as locally derived;
- do not call the local hash an upstream-authenticated checksum.

---

## 89. Generated Artifact Provenance

Persist safe provenance such as:

```text
producer component
producer processor
input loader release
upstream expected digest if present
computed local digest
canonical managed destination
```

---

## 90. Safe Shared Publication

Reusable immutable generated outputs are published only after validation.

Conceptual:

```text
processor staging output
      |
      v
verify
      |
      v
safe temporary shared destination
      |
      v
atomic publication
```

A failed instance publication may leave an unreferenced verified shared artifact, but never a
corrupt canonical file.

---

## 91. Generated Output Reuse

Before rerunning a processor, Graphene may reuse an existing canonical generated output only when:

- path is managed;
- identity matches;
- digest validates;
- provenance matches required component/release.

Filename existence alone is not sufficient.

This reuse must be covered by integration tests.

---

# Install Request, Plan, Execution, Receipt

## 92. Install Request Evolution

The current Phase 1 `InstallRequest` is Vanilla-oriented.

Preferred compatibility:

```text
existing InstallRequest
    -> remains Vanilla convenience

new component-aware request
    -> base Minecraft + exact/dynamic loader selection
```

Possible names:

```text
ComponentInstallRequest
InstallTargetRequest
```

Do not break the Vanilla API merely to add loaders.

---

## 93. Component-Aware Planning

Potential facade:

```text
install().plan(vanilla_request)
install().plan_components(component_request)
```

or a compatible unified builder.

The executor must receive exact resolved component state.

It must not query providers.

---

## 94. Install Plan Version

Phase 3 likely requires plan schema/version evolution.

Potential additions:

```text
resolved component set
preparation inputs
embedded inputs
processor steps
generated outputs
```

Vanilla plans remain valid in the new model.

---

## 95. Deterministic Plans

The same frozen provider/base inputs must produce byte-equivalent normalized plan data.

Do not put into plan identity:

- staging paths;
- operation IDs;
- timestamps;
- process IDs;
- unordered map iteration;
- network timing.

---

## 96. Planning Pipeline

Recommended:

```text
validate request
 -> resolve base Minecraft
 -> resolve exact loader
 -> build component graph
 -> compose patches
 -> resolve final libraries/artifacts
 -> normalize preparation recipe
 -> enumerate remote inputs
 -> enumerate generated outputs
 -> validate managed paths
 -> build InstallPlan
```

No committed mutation.

---

## 97. Execution Pipeline

Recommended sequence:

1. validate plan version;
2. acquire instance-create lock;
3. ensure final instance target does not already exist;
4. create instance staging;
5. create loader-tool staging;
6. acquire all remote artifacts;
7. materialize immutable inputs;
8. extract declared embedded installer inputs;
9. prepare isolated processor input tree;
10. run required processors in deterministic order;
11. validate generated outputs;
12. publish reusable generated shared artifacts;
13. materialize final instance-local state;
14. reuse existing safe native extraction;
15. write updated receipt;
16. validate staged instance;
17. seal cancellation only before the final non-interruptible publication;
18. publish instance;
19. clean staging;
20. release locks.

---

## 98. Cancellation Point of No Return

Cancellation remains effective during:

- provider resolution;
- artifact acquisition;
- installer parsing;
- embedded extraction;
- processor execution;
- output hashing;
- staged validation.

Do not seal cancellation before:

- filesystem lock waits;
- processor execution;
- large copies;
- output verification.

Only the smallest final publication section is non-cancellable.

---

## 99. Loader Progress

Example:

```text
Install Forge Instance
├── Resolve Minecraft             done
├── Resolve loader                done
├── Compose components            done
├── Acquire artifacts             71%
├── Prepare loader
│   ├── Extract inputs            done
│   ├── Processor 1               done
│   ├── Processor 2               active
│   └── Verify outputs            waiting
├── Materialize instance          waiting
├── Validate                      waiting
└── Commit                        waiting
```

Fabric can omit processor stages.

---

## 100. Processor Child Operations

Long processor steps may become child operations.

Requirements:

- parent cancellation propagates;
- one terminal state;
- bounded event volume;
- deterministic parent aggregation.

---

## 101. Resolution Concurrency

Independent metadata queries may run concurrently where safe.

Patch composition order must remain deterministic.

Artifact acquisition keeps Phase 0 bounded concurrency.

---

## 102. Duplicate Work Gates

Keyed gates may deduplicate:

```text
exact loader release metadata
generated canonical artifact
```

Cross-process publication remains protected by filesystem-safe semantics.

---

## 103. Final `ResolvedMinecraft`

After composition it still exposes normal Phase 1 launch data:

```text
main class
client
libraries
assets
logging
JVM args
game args
Java requirement
base Minecraft version
```

Phase 3 adds component identity/provenance.

No loader-specific launch type is required.

---

## 104. Install Receipt Evolution

The receipt must persist exact components.

Recommended addition:

```text
components:
  - uid
  - exact version
  - kind
  - safe provenance/provider identity
```

It still stores final normalized launch metadata.

---

## 105. Receipt Schema Versioning

Evaluate a schema/version bump when component identity is added.

If bumped:

- old schema remains readable where practical;
- migration is deterministic;
- tests cover it;
- API docs explain it.

---

## 106. Phase 1 Vanilla Receipt Compatibility

Preferred backward behavior:

```text
old Vanilla receipt without components
       |
       v
interpret in memory as:
net.minecraft <resolved Minecraft version>
```

Launching an old Vanilla instance should not require network or destructive rewrite.

---

## 107. Launch Invariant

After commit:

```text
receipt
  |
  v
LaunchService
  |
  v
LaunchPlan
```

No provider access.

No installer parse.

No Fabric/Forge/NeoForge dispatch.

---

## 108. Forbidden Launch Branches

Reject code patterns equivalent to:

```rust
if loader == "fabric" { ... }
match loader_kind { Forge =>..., NeoForge =>...}
```

inside `graphene-launch`.

Loader differences are already represented by:

```text
main class
classpath
arguments
Java requirement
```

---

## 109. Authentication Isolation

Phase 2 remains independent:

```text
Account -> LaunchSession
Components -> ResolvedMinecraft / receipt
Java -> JavaRuntime
```

They meet only at launch planning.

Loader providers never receive account tokens.

---

## 110. Managed Java Integration

Game Java comes from final composed metadata.

Processor Java comes from preparation requirements.

Both use provider-neutral Java APIs.

Forge/NeoForge code must not call the Adoptium adapter directly.

---

## 111. Loader Changes Before Phase 4

Phase 3 primarily supports creating new loader-backed instances.

In-place upgrade/downgrade belongs to Phase 4 target-state mutation.

However, all Phase 3 primitives must make a future change expressible as:

```text
desired component set
 -> resolve
 -> plan
 -> transactional mutation
```

not provider-specific `install_into_existing_instance()` calls.

---

## 112. Support Query API

Hosts should be able to inspect support before an expensive install.

Potential normalized result:

```text
Supported
MetadataOnly(reason)
Unsupported(reason)
```

Reasons should be structured.

---

# Errors, Diagnostics, Security

## 113. Error Contract Expansion

Suggested families:

```text
COMPONENT_INVALID
COMPONENT_GRAPH_INVALID
COMPONENT_CYCLE
COMPONENT_REQUIREMENT_UNSATISFIED
COMPONENT_CONFLICT
COMPONENT_PATCH_INVALID
COMPONENT_JAVA_CONFLICT

LOADER_PROVIDER_UNAVAILABLE
LOADER_VERSION_NOT_FOUND
LOADER_VERSION_UNSUPPORTED
LOADER_METADATA_INVALID
LOADER_PROFILE_INVALID
LOADER_INSTALLER_INVALID
LOADER_INSTALLER_SPEC_UNSUPPORTED
LOADER_ARTIFACT_UNVERIFIABLE

LOADER_PROCESSOR_UNSUPPORTED
LOADER_PROCESSOR_PLACEHOLDER_INVALID
LOADER_PROCESSOR_JAVA_UNAVAILABLE
LOADER_PROCESSOR_FAILED
LOADER_PROCESSOR_TIMEOUT
LOADER_PROCESSOR_OUTPUT_MISSING
LOADER_PROCESSOR_OUTPUT_MISMATCH
LOADER_PROCESSOR_CANCELLED
```

Exact naming follows existing style.

---

## 114. Safe Error Context

May include:

```text
loader family
exact loader version
base Minecraft version
component UID
processor normalized ID/index
safe Maven coordinate
expected/actual digest
```

Must not include:

- auth tokens;
- arbitrary provider bodies;
- unrestricted process output;
- unsafe host paths.

---

## 115. Loader Diagnostics

Potential diagnostic codes:

```text
LOADER_VERSION_INCOMPATIBLE
LOADER_PROFILE_SCHEMA_UNSUPPORTED
LOADER_PROCESSOR_REQUIRES_JAVA
LOADER_GENERATED_OUTPUT_CORRUPT
LOADER_RELEASE_METADATA_UNAVAILABLE
LOADER_LEGACY_SUPPORT_LIMITED
```

Diagnostics remain data, not localized prose.

---

## 116. Resource Limits

Define explicit limits for:

- loader version list;
- Maven metadata;
- profile JSON;
- installer JSON;
- installer archive;
- archive entry count;
- embedded input;
- processor count;
- classpath length;
- argument count/length;
- generated output count;
- captured process output.

Oversized inputs fail early.

---

## 117. Maven XML Safety

If Maven metadata XML is required:

- use maintained bounded XML parsing;
- disable/avoid external entities;
- no DTD/network resolution;
- parse only required fields;
- deterministic output;
- no regex-as-XML-parser.

---

## 118. Redirect / TLS Safety

Production loader/provider URLs require HTTPS under normal policy.

Keep existing redirect/downgrade protections.

Fixture local HTTP is explicit test-only configuration.

Provider metadata must not disable TLS validation.

---

## 119. Repository URL Validation

Loader-provided Maven repositories are untrusted metadata.

Validate:

- scheme;
- URL length;
- production HTTPS policy;
- no `file:` URLs;
- no uncontrolled local/private endpoint injection;
- existing redirect rules.

Any deliberate legacy HTTP support requires explicit policy and diagnostics.

---

## 120. Integrity Classes

Keep provenance distinct:

```text
trusted upstream SHA-256/SHA-512
trusted legacy upstream SHA-1
verified-container embedded input
locally derived generated digest
```

Never blur these trust classes.

---

## 121. Installer Provenance

Persist safe provenance:

```text
loader family
exact loader version
provider ID
installer identity/coordinate
installer digest
installer schema family
```

Do not persist the entire raw installer DTO as the instance source of truth.

---

## 122. Processor Artifact Provenance

Plans retain normalized processor/tool coordinates and verified identity.

This enables:

- deterministic snapshots;
- diagnostics;
- future repair;
- generated output provenance.

---

## 123. Installer Archive Security

Never:

- extract arbitrary archive trees;
- honor absolute/traversal paths;
- create archive symlinks;
- preserve unsafe special permissions;
- execute embedded scripts.

Read/extract only declared safe inputs.

---

## 124. Script Prohibition

Phase 3 client installation must not execute installer-directed:

```text
.sh
.bat
.cmd
.ps1
arbitrary native executable
```

Only verified Java processor artifacts run through the explicit tool-runner boundary.

---

## 125. Argument Safety

Processor args are direct argv tokens.

Validate:

- NUL forbidden;
- bounded count;
- bounded length;
- path placeholders contained;
- data variables typed;
- no shell interpretation.

Spaces are ordinary argument contents.

---

## 126. Output Path Safety

Each output is classified:

```text
staging-only
shared immutable
instance staging
```

Reject output targets into:

- user home arbitrary files;
- Graphene config;
- secret storage;
- another instance;
- undeclared cache paths.

---

## 127. Shared vs Instance Scope

Globally reusable generated loader libraries should normally be shared immutable artifacts.

Instance-specific files remain in instance staging.

Do not duplicate global generated classpath artifacts in every instance without upstream need.

---

## 128. Final Staged Validation

Before commit verify:

- final component set;
- final metadata;
- receipt;
- client artifact;
- libraries;
- assets/logging;
- generated outputs;
- natives;
- main class;
- Java requirement;
- no duplicate classpath target;
- no unresolved placeholders;
- all preparation outputs present.

---

## 129. Loader Metadata Persistence

Normalized/frozen provider metadata may be cached for provenance/repair.

It remains rebuildable cache.

The committed receipt must itself be sufficient for offline launch.

---

## 130. Offline Launch Requirement

For every supported loader fixture:

1. install;
2. disconnect fixture provider;
3. restart Graphene;
4. read committed state;
5. select Java;
6. create `LaunchSession`;
7. build `LaunchPlan`.

No loader provider access is allowed in this path.

---

# Service / Facade / Builder

## 131. Loader Version Operations

Version listing is a cancellable operation.

Suggested stages:

```text
validate
fetch
normalize
filter support
complete
```

---

## 132. Exact Resolution Operation

Potential stages:

```text
resolve release
acquire profile/installer metadata
verify
normalize component
validate compatibility
complete
```

No mutation.

---

## 133. Builder Changes

`GrapheneBuilder` may accept:

```text
loader registry
Fabric provider configuration
Forge provider configuration
NeoForge provider configuration
```

Production defaults may use official origins.

Fixtures must override all origins without global mutable state.

---

## 134. Service Context

May gain:

```text
loader provider registry
loader-resolution keyed gates
generic install-tool runner
loader provider policy/config
```

Prefer a registry over concrete provider fields when it keeps orchestration generic.

---

## 135. Facade Direction

Potential normalized facade:

```text
graphene.minecraft().loader_versions(...)
graphene.minecraft().resolve_loader(...)
graphene.install().plan_components(...)
```

or a dedicated `loaders()` service.

Whichever is chosen:

- DTOs stay private;
- exact versions are explicit;
- execution remains through the installation service.

---

## 136. Public Fabric Example

Directional example:

```rust
let versions = graphene
.minecraft()
.loader_versions(LoaderKind::Fabric, minecraft_version)
.await?;
```

Then build a component-aware install request with an exact version.

Signatures may adapt to repository style.

---

## 137. Public Forge/NeoForge Boundary

Hosts should not know:

```text
install_profile.json
processor data maps
binpatch details
installer JAR internals
MCP/NeoForm tool command lines
```

A host chooses:

```text
Minecraft version
loader family
loader release
```

Graphene performs normalized preparation.

---

## 138. No Primary Provider-Specific Install API

Avoid designing the engine around:

```text
install_fabric(...)
install_forge(...)
install_neoforge(...)
```

The stable path is component-aware installation.

Convenience wrappers may delegate later without becoming separate engines.

---

# Performance / Reuse / Concurrency

## 139. Metadata Caching

Use existing network/cache mechanisms where appropriate.

Do not load unbounded Maven/release history into memory.

---

## 140. Processor Input Copies

Mutation isolation may require copies.

Safe future optimizations such as reflink/copy-on-write are allowed only if cache immutability is
preserved.

Writable hardlinks are forbidden.

---

## 141. Generated Output Cache

A second instance using the same exact loader release should reuse already validated generated
outputs when possible.

This is a Phase 3 acceptance scenario.

---

## 142. Processor Retry

Do not automatically rerun arbitrary failed processors.

Baseline:

```text
network acquisition -> existing bounded retry
deterministic processor failure -> terminal
```

Any retry starts from clean step outputs.

Never reuse partial failed output.

---

## 143. Component Metadata Cache

Provider component metadata may be cached.

Correctness does not depend on a database-only record.

Exact components remain in committed files.

---

# Fixtures and Tests

## 144. Fixture Layout

Recommended:

```text
tests/fixtures/loaders/
├── fabric/
│   ├── versions/
│   ├── profiles/
│   └── maven/
├── forge/
│   ├── legacy/
│   ├── modern/
│   ├── installers/
│   ├── maven/
│   └── processor-tools/
├── neoforge/
│   ├── modern/
│   ├── installers/
│   ├── maven/
│   └── processor-tools/
├── hostile/
└── expected/
```

All fixtures use pinned exact identities.

---

## 145. Component Graph Unit Tests

Required:

- UID validation;
- version bounds;
- duplicate components;
- missing base;
- multiple bases;
- multiple primary loaders;
- missing requirement;
- satisfied requirement;
- conflict;
- cycle;
- deterministic order;
- graph bounds.

---

## 146. Patch Merge Tests

Required:

- scalar override;
- scalar preserve;
- library append;
- library replacement by Maven identity;
- deterministic order;
- JVM args;
- game args;
- rules preserved;
- equal Java merge;
- Java conflict;
- final required-field validation.

---

## 147. Fabric Provider Tests

Required:

- version list;
- exact lookup;
- unsupported pair;
- stability metadata;
- profile normalization;
- main class;
- libraries;
- args;
- malformed JSON;
- missing fields;
- oversized response;
- invalid URL;
- base/profile mismatch.

---

## 148. Fabric Integration

Fixture path:

```text
Mojang base fixture
+
Fabric fixture
 -> graph
 -> composed ResolvedMinecraft
 -> InstallPlan
 -> committed instance
 -> offline LaunchPlan
```

Normal Fabric fixture should require no processor step.

---

## 149. Forge Legacy Tests

If Tier A:

- valid pinned legacy profile;
- base validation;
- legacy metadata;
- libraries/artifact;
- legacy args;
- malformed profile;
- ambiguous schema;
- unsupported variant.

---

## 150. Forge Modern Tests

Required:

- release discovery;
- exact release;
- installer integrity;
- verified parse;
- version JSON normalization;
- install profile normalization;
- data variables;
- side filtering;
- processor dependencies;
- outputs;
- unknown spec;
- malformed processor;
- unknown placeholder;
- base mismatch.

---

## 151. NeoForge Tests

Required:

- Maven/version discovery;
- multiple version-scheme fixtures where relevant;
- exact base validation;
- checksum sidecar;
- installer parse;
- patch;
- processors;
- malformed metadata;
- unknown spec;
- unsupported platform/base.

---

## 152. Placeholder Expansion Tests

Required:

```text
literal
ROOT
INSTALLER
LIBRARY_DIR
MINECRAFT_JAR
MINECRAFT_VERSION
SIDE
data variable
Maven value
missing variable
unknown placeholder
escape attempt
NUL
length/count bounds
```

---

## 153. Tool Runner Tests

Use fake Java/tool processes.

Cover:

- success;
- nonzero exit;
- spawn failure;
- timeout;
- cancellation;
- stdout flood;
- stderr flood;
- invalid encoding;
- bounded process termination;
- explicit Java executable;
- no shell.

---

## 154. Generated Output Tests

Cover:

- exists;
- missing;
- SHA-1 match;
- SHA-256 match;
- mismatch;
- invalid zero-size where relevant;
- wrong file type;
- output escape;
- local digest;
- valid reuse;
- invalid existing output regeneration.

---

## 155. Hostile Installer Fixtures

Required:

```text
ZIP traversal
absolute path
oversized metadata
too many entries
oversized embedded input
malformed JSON
duplicate/conflicting entry
unknown placeholder
output outside root
NUL argument
unsafe script reference
unsupported executable type
TLS/URL downgrade attempt
```

---

## 156. Network-Normalized Processor Regression

If a supported provider converts a known network-fetch processor into a Graphene artifact, add a
fixture proving semantic equivalence and offline processor execution.

Do not add this optimization based on string heuristics.

---

## 157. Install Plan Snapshots

Required normalized snapshots:

```text
Vanilla plan
Fabric plan
Forge modern plan
NeoForge plan
Forge legacy plan if Tier A
```

Redact execution-only staging data.

---

## 158. Resolved Minecraft Snapshots

Snapshot final composed metadata for every loader.

Differences should be normalized launch metadata:

```text
main class
libraries
arguments
Java requirement
components
```

not DTOs.

---

## 159. Receipt Snapshots

Snapshot committed receipt schemas for:

```text
Vanilla
Fabric
Forge
NeoForge
```

No temporary path, auth secret, or raw provider body.

---

## 160. Receipt Migration Tests

Required:

- read Phase 1 receipt;
- infer base Minecraft component;
- rebuild same launch plan;
- no provider/network;
- no destructive rewrite;
- malformed old receipt fails safely.

---

## 161. Cross-Provider Convergence Test

A common downstream launch harness receives only:

```text
InstallReceipt
JavaRuntime
LaunchSession
```

for all loader families.

It must not import provider-private modules.

---

## 162. Architecture Tests

Expand architecture checks to enforce:

- no `graphene-launch -> graphene-providers`;
- no `graphene-install -> graphene-providers`;
- no `graphene-minecraft -> graphene-providers`;
- loader DTO modules private;
- no new UI dependencies;
- no direct Reqwest in domain/install code;
- thin crate roots;
- service does not import private DTOs;
- graph code is outside `lib.rs`;
- processor code is outside `lib.rs`;
- graph remains acyclic.

Do not weaken earlier checks.

---

## 163. Crate-Root Guard

Prefer semantic guards:

```text
no provider DTO structs in lib.rs
no HTTP implementation in lib.rs
no processor loop in lib.rs
no graph algorithm in lib.rs
```

A conservative size threshold may supplement but not replace semantic checks.

---

## 164. Fuzz Targets

High-value targets:

- Fabric profile JSON;
- Forge install profile;
- Forge version JSON;
- NeoForge install profile;
- Maven XML;
- installer ZIP entry selection;
- processor placeholder parser;
- component graph serialized input if public.

Keep production resource limits enabled.

---

## 165. Property Tests

Useful properties:

- topological result respects every edge;
- deterministic input -> deterministic order;
- merge deterministic;
- managed path expansion cannot escape root;
- placeholder expansion cannot introduce NUL;
- publication targets are declared;
- old Vanilla receipt migration preserves launch semantics.

---

# Cross-Platform Requirements

## 166. CI Matrix

Required portable testing on:

```text
Linux
Windows
macOS
```

Cover graph/provider normalization/plan/archive/path/process/receipt behavior.

---

## 167. Windows Concerns

Test:

- path prefixes/drives;
- path separator rules;
- classpath separator;
- Java path containing spaces;
- no `.bat`/shell execution;
- bounded process output/cancellation.

---

## 168. macOS Concerns

Test:

- Java bundle executable paths;
- archive permission handling;
- path normalization;
- processor cancellation.

---

## 169. Linux Concerns

Test:

- executable permission on fixtures;
- process termination;
- path containment;
- archive permissions.

---

# Real Smoke / Support Documentation

## 170. Fabric Smoke Test

Implementation must create `docs/PHASE_3_FABRIC_SMOKE_TEST.md`.

Use pinned exact versions and verify:

1. clean data root;
2. exact loader resolution;
3. graph inspection;
4. composed metadata;
5. install plan;
6. installation;
7. restart;
8. offline launch planning;
9. launch;
10. Fabric initialization evidence;
11. clean termination;
12. second instance shared-artifact reuse.

---

## 171. Forge Smoke Test

Create `docs/PHASE_3_FORGE_SMOKE_TEST.md`.

Verify:

- exact release;
- verified installer;
- normalized patch;
- processor plan;
- processor Java;
- staged execution;
- generated output validation;
- commit;
- restart;
- offline launch;
- Forge initialization;
- generated-output reuse.

If multiple Forge schema families are claimed Tier A, sign-off evidence must reflect that claim.

---

## 172. NeoForge Smoke Test

Create `docs/PHASE_3_NEOFORGE_SMOKE_TEST.md`.

Verify:

- exact release;
- official installer/integrity;
- component;
- processor recipe;
- Java integration;
- generated outputs;
- receipt;
- restart/offline launch;
- NeoForge initialization;
- reuse.

---

## 173. Smoke-Test Honesty

When prerequisites are unavailable, record:

```text
Status: NOT RUN
Reason: <specific prerequisite>
```

Fixture success is not a real smoke pass.

---

## 174. Support Matrix

Create `docs/PHASE_3_SUPPORT_MATRIX.md`.

Record:

```text
loader family
pinned fixture release
Minecraft base
profile/installer family
Tier A / Tier B / unsupported
required Java
real smoke status
known limitations
```

This is evidence-based, not aspirational.

---

# Dependencies, ADRs, and Evolution

## 175. Dependency Addition Policy

Potential new dependencies may cover:

- bounded XML parsing;
- archive helpers;
- process utilities;
- provider-specific version parsing.

For each dependency ask:

1. which crate owns it?
2. is it maintained?
3. cross-platform?
4. safe on untrusted input?
5. native/runtime baggage?
6. is an existing dependency enough?
7. does it create a forbidden edge?

---

## 176. External Protocol Verification

At implementation time, verify current behavior against primary official sources:

```text
Fabric official Meta/documentation/source
MinecraftForge official source/Maven/Installer
NeoForged official source/Maven/InstallerTools
```

Third-party launchers are secondary compatibility references only.

---

## 177. Provider Distribution Policy

Use official loader artifacts and documented distribution paths.

Do not scrape advertisement pages to obtain binaries.

Document provider artifact-source policy and any upstream distribution/license constraints in the
provider ADR/security review.

---

## 178. Component Graph ADR

Record:

- why graph lives in `graphene-minecraft`;
- base Minecraft component;
- primary-loader conflicts;
- deterministic order;
- patch composition;
- launch neutrality.

---

## 179. Loader Registry ADR

Record:

- provider interface ownership;
- registry ownership;
- capabilities;
- exact resolution;
- extension path for future loaders;
- why service code does not branch everywhere.

---

## 180. Processor ADR

Record:

- why Graphene does not blindly run installers on committed state;
- normalized recipe;
- staging;
- Java tool-runner boundary;
- output verification;
- network policy;
- executable-code trust limitation.

---

## 181. Receipt Schema ADR

If receipt schema changes, record:

- version bump;
- exact components;
- Phase 1 migration;
- backward launch;
- future Phase 4 relationship.

---

## 182. Public API Stability

High-stability candidates:

```text
ComponentUid
ComponentVersion
ComponentKind
ComponentDescriptor
ResolvedComponent
LoaderKind
LoaderVersion
LoaderVersionSummary
LoaderSelection
LoaderSupport
```

Medium stability:

```text
provider capabilities
component request builders
preparation summaries
```

Private:

```text
Fabric DTOs
Forge DTOs
NeoForge DTOs
Maven XML structs
raw processor-profile structs
archive-entry types
provider-specific version parsers
```

---

## 183. DTO Privacy Test

Consumer compile tests should be able to use normalized loader/component APIs without importing
provider DTO modules.

Provider DTO modules should remain private.

---

# Future-Phase Compatibility

## 184. Phase 4 Target-State Compatibility

Phase 4 will compare desired and current component state.

Expected future:

```text
desired components
+
current receipt/lock
      |
      v
component diff
      |
      v
Install/RepairPlan
```

Phase 3 must not hide loader identity in synthetic Minecraft IDs.

---

## 185. Phase 4 Loader Upgrade

Future:

```text
Fabric A -> Fabric B
Forge A  -> Forge B
```

must be a component target diff.

It must not call provider-specific mutation APIs.

---

## 186. Phase 5 Content Compatibility

Phase 5 needs:

```text
Minecraft base
loader family
exact loader version
```

for mod compatibility.

Phase 3 receipt/components provide that context.

---

## 187. Fabric API and Phase 5

Fabric API remains content.

Phase 5 may resolve it as a dependency.

Phase 3 must not hide it in loader installation.

---

## 188. Phase 6 Modpacks

Pack formats that declare Minecraft+loader normalize into Phase 3 component requests.

They must not invoke a separate loader installer.

---

## 189. Future Quilt

Adding Quilt should primarily require:

```text
provider adapter
normalization
fixtures
```

If Quilt requires `graphene-launch` changes, revisit the component boundary.

---

## 190. Future Legacy Fabric

Legacy Fabric can have different metadata behavior while still producing:

```text
ResolvedComponent
MinecraftVersionPatch
ComponentPreparationRecipe if needed
```

Do not make current Fabric profile shape a universal domain rule.

---

## 191. Future Unusual Loaders

Unusual loaders may require new explicit preparation action types.

Do not add a generic "run arbitrary command" escape hatch for future-proofing.

Every new executable action type requires a security review.

---

# Workstreams

## 192. Workstream 1 — Prior Gate and Component Domain

Implement:

- run prior quality gates;
- component value types;
- graph;
- graph tests;
- patch model;
- merge tests;
- architecture guards.

**Exit:** synthetic component composition works without providers.

---

## 193. Workstream 2 — Vanilla Base Component

Implement:

- represent Phase 1 base Minecraft as component;
- resolved component set;
- preserve Vanilla request path;
- snapshots.

**Exit:** Vanilla behavior remains equivalent except for explicit component identity.

---

## 194. Workstream 3 — Loader Provider Registry

Implement:

- interface;
- capabilities;
- registry;
- service queries;
- fixture injection;
- builder wiring.

**Exit:** synthetic loader provider works without use-case branching.

---

## 195. Workstream 4 — Fabric Vertical Slice

Implement:

- Fabric config;
- DTOs;
- versions;
- exact resolve;
- profile normalization;
- artifacts;
- patch;
- component-aware plan;
- fixture install;
- offline launch.

**Exit:** first non-Vanilla loader proves unchanged launch core.

---

## 196. Workstream 5 — Forge Installer Normalization

Implement:

- installer artifact;
- bounded JAR parsing;
- schema detection;
- data values;
- embedded inputs;
- processors;
- outputs;
- placeholder model.

**Exit:** a pinned Forge installer becomes a deterministic preparation recipe without executing it.

---

## 197. Workstream 6 — Generic Processor Execution

Implement:

- runner port;
- Java adapter;
- staging root;
- input isolation;
- placeholder expansion;
- execution;
- timeout;
- cancellation;
- bounded output;
- output verification;
- generated publication.

**Exit:** synthetic verified processor safely produces a declared generated artifact.

---

## 198. Workstream 7 — Forge Vertical Slice

Implement:

- discovery;
- exact release;
- installer normalization;
- selected schema families;
- patch;
- preparation;
- receipt;
- offline launch.

**Exit:** pinned Forge fixture reaches loader-neutral launch plan.

---

## 199. Workstream 8 — NeoForge Vertical Slice

Implement:

- discovery/version mapping;
- exact release;
- integrity;
- installer normalization;
- patch;
- preparation;
- receipt;
- offline launch.

**Exit:** pinned NeoForge fixture uses the same generic downstream pipeline.

---

## 200. Workstream 9 — Receipt Migration and Reuse

Implement:

- receipt schema evolution;
- old Vanilla compatibility;
- exact component persistence;
- generated provenance;
- generated reuse.

**Exit:** old Vanilla still launches and repeated loader installs reuse validated shared output.

---

## 201. Workstream 10 — Hardening

Implement:

- hostile fixtures;
- race/cancellation tests;
- resource limits;
- architecture checks;
- support matrix;
- API docs;
- security review;
- ADRs;
- smoke procedures.

**Exit:** automated exit checklist green; any unavailable real smoke remains explicitly blocking
final
`DONE`.

---

## 202. Recommended Implementation Order

```text
1. prior-phase gate
2. component values
3. graph
4. patch model
5. Vanilla base component
6. provider registry
7. component-aware request/plan
8. Fabric provider
9. Fabric fixture end-to-end
10. Forge installer parser
11. preparation recipe
12. tool-runner port
13. processor execution
14. generated output publication
15. Forge provider
16. Forge fixture end-to-end
17. NeoForge provider
18. NeoForge fixture end-to-end
19. receipt migration
20. reuse/cache
21. diagnostics/security
22. architecture guards
23. docs/ADRs
24. real smoke tests
```

Do not implement three providers first and postpone the stable component graph.

---

# Review Checklists

## 203. Architecture Review

- [ ] graph lives in stable domain.
- [ ] DTOs remain adapter-private.
- [ ] install has no provider dependency.
- [ ] launch has no loader/provider dependency.
- [ ] provider registry exists.
- [ ] service does not spread loader-name branches.
- [ ] crate roots are thin.
- [ ] graph is acyclic.
- [ ] no UI dependency.
- [ ] architecture checks enforce new edges.

---

## 204. Component Review

- [ ] exactly one Minecraft base.
- [ ] stable component UIDs.
- [ ] opaque versions.
- [ ] primary-loader conflict.
- [ ] requirement validation.
- [ ] cycle rejection.
- [ ] deterministic order.
- [ ] documented patch semantics.
- [ ] base Minecraft identity preserved.
- [ ] exact components in resolved state.

---

## 205. Fabric Review

- [ ] exact version resolution.
- [ ] official current Meta contract verified at implementation time.
- [ ] private DTOs.
- [ ] profile -> patch.
- [ ] libraries -> Artifact pipeline.
- [ ] no fake installer process.
- [ ] no implicit Fabric API.
- [ ] fixture -> offline launch plan.

---

## 206. Forge Review

- [ ] official-compatible discovery.
- [ ] installer verified before parse.
- [ ] schema family explicit.
- [ ] legacy scope precise if supported.
- [ ] modern profile normalized.
- [ ] client processors filtered.
- [ ] processor deps declared.
- [ ] outputs verified.
- [ ] unknown spec rejected.
- [ ] support matrix honest.

---

## 207. NeoForge Review

- [ ] version discovery handles current scheme.
- [ ] base compatibility validated.
- [ ] official checksum used where available.
- [ ] installer verified.
- [ ] DTOs private.
- [ ] generic preparation recipe.
- [ ] no provider-specific branch in install executor.
- [ ] fixture -> offline launch plan.

---

## 208. Processor Review

- [ ] all inputs declared.
- [ ] all dependencies acquired first.
- [ ] explicit Java executable.
- [ ] no shell.
- [ ] staging root.
- [ ] immutable cache isolation.
- [ ] client side rules.
- [ ] allowlisted placeholders.
- [ ] contained paths.
- [ ] timeout.
- [ ] cancellation.
- [ ] bounded output.
- [ ] outputs declared.
- [ ] upstream hashes enforced.
- [ ] local hashes labeled correctly.
- [ ] no arbitrary script.
- [ ] security review admits no JVM sandbox.

---

## 209. Persistence Review

- [ ] exact components persisted.
- [ ] final launch metadata persisted.
- [ ] receipt schema explicit.
- [ ] Phase 1 receipt compatibility tested.
- [ ] raw provider DTO not source of truth.
- [ ] generated provenance recorded.
- [ ] loader provider not required for offline launch.

---

## 210. Security Review

- [ ] metadata bounded.
- [ ] XML safe.
- [ ] installer archive bounded.
- [ ] traversal rejected.
- [ ] unsafe links unused/rejected.
- [ ] installer verified before parse.
- [ ] processor artifacts verified.
- [ ] no shell.
- [ ] no scripts.
- [ ] no auth-secret injection.
- [ ] output paths constrained.
- [ ] timeout/cancel.
- [ ] output mismatch fails.
- [ ] cache immutable.
- [ ] TLS/downgrade policy preserved.
- [ ] legacy integrity limitations documented.
- [ ] executable-code trust boundary documented.

---

## 211. Test / CI Review

- [ ] graph tests.
- [ ] patch tests.
- [ ] Fabric tests.
- [ ] Fabric integration.
- [ ] Forge legacy tests if Tier A.
- [ ] Forge modern tests.
- [ ] processor tests.
- [ ] hostile installer tests.
- [ ] NeoForge tests.
- [ ] receipt migration.
- [ ] generated-output reuse.
- [ ] cancellation/race tests.
- [ ] architecture checks.
- [ ] Linux.
- [ ] Windows.
- [ ] macOS.
- [ ] format/check/test/clippy/doc.
- [ ] smoke records accurate.

---

## 212. Phase 3 Exit Checklist

Phase 3 may be marked complete only when all applicable items have executable evidence.

### Prior-Phase Gate

- [ ] Phase 0 automated gates pass.
- [ ] Phase 1 automated gates pass.
- [ ] Phase 2 automated gates pass.
- [ ] required prior real-smoke release gates are satisfied.
- [ ] no prior smoke evidence is fabricated.

### Architecture

- [ ] component graph exists.
- [ ] loader provider registry exists.
- [ ] no forbidden dependency edge.
- [ ] no UI dependency.
- [ ] crate roots remain thin.
- [ ] DTOs private.
- [ ] architecture checker covers Phase 3.

### Component Model

- [ ] base Minecraft is a component.
- [ ] exact identities exist.
- [ ] primary loader conflict is structural.
- [ ] requirements validate.
- [ ] cycles reject.
- [ ] order deterministic.
- [ ] patches deterministic.
- [ ] Java conflicts structured.
- [ ] `ResolvedMinecraft` includes exact components.

### Installation API

- [ ] Vanilla request path remains usable.
- [ ] component-aware request exists.
- [ ] exact loader resolved before execution.
- [ ] plan represents preparation.
- [ ] executor does not query providers.
- [ ] plan snapshots deterministic.

### Fabric

- [ ] version list fixture.
- [ ] exact resolve fixture.
- [ ] profile normalization.
- [ ] library resolution.
- [ ] no implicit Fabric API.
- [ ] fixture install.
- [ ] offline launch from receipt.
- [ ] smoke procedure.
- [ ] real smoke pass for final sign-off.

### Forge

- [ ] exact supported release.
- [ ] verified installer.
- [ ] schema detection.
- [ ] modern normalization.
- [ ] legacy family if claimed.
- [ ] client processor filtering.
- [ ] processor dependencies.
- [ ] processor Java.
- [ ] outputs verified.
- [ ] fixture install.
- [ ] offline launch.
- [ ] smoke procedure.
- [ ] required real smoke pass.

### NeoForge

- [ ] exact supported release.
- [ ] version/base validation.
- [ ] trusted integrity.
- [ ] installer normalization.
- [ ] client processors.
- [ ] outputs.
- [ ] fixture install.
- [ ] offline launch.
- [ ] smoke procedure.
- [ ] real smoke pass.

### Processor Pipeline

- [ ] provider-neutral recipe.
- [ ] executable JARs verified.
- [ ] classpath verified.
- [ ] isolated staging.
- [ ] cache mutation isolation.
- [ ] allowlisted placeholders.
- [ ] no shell.
- [ ] no arbitrary script.
- [ ] correct side filtering.
- [ ] timeout.
- [ ] cancellation.
- [ ] no stdout/stderr deadlock.
- [ ] bounded output.
- [ ] output path containment.
- [ ] upstream hashes enforced.
- [ ] local provenance explicit.
- [ ] safe publication.
- [ ] valid reuse.

### Receipt / Launch

- [ ] exact components persist.
- [ ] schema migration documented.
- [ ] old Vanilla receipt compatible as planned.
- [ ] offline launch.
- [ ] launch has no loader branch.
- [ ] provider not needed after commit.
- [ ] auth boundary unchanged.
- [ ] Java boundary unchanged.

### Security

- [ ] metadata limits.
- [ ] safe XML.
- [ ] hostile installer fixtures.
- [ ] integrity policy documented.
- [ ] processor trust boundary documented.
- [ ] uncontrolled network behavior not silently accepted.
- [ ] auth secrets absent from processor.
- [ ] filesystem escape tests.
- [ ] no script execution.
- [ ] Phase 3 security review complete.

### Tests / Tooling

- [ ] unit tests.
- [ ] integration tests.
- [ ] snapshots.
- [ ] fuzz/property targets as planned.
- [ ] architecture tests.
- [ ] Windows CI.
- [ ] Linux CI.
- [ ] macOS CI.
- [ ] `cargo fmt`.
- [ ] `cargo check`.
- [ ] `cargo test`.
- [ ] `cargo clippy -D warnings`.
- [ ] `cargo doc`.
- [ ] support matrix matches evidence.
- [ ] smoke records match evidence.

---

## 213. Required Implementation Documentation

Phase 3 implementation must create/update:

```text
docs/PHASE_3_IMPLEMENTATION_PLAN.md
docs/PHASE_3_API.md
docs/PHASE_3_SECURITY_REVIEW.md
docs/PHASE_3_SUPPORT_MATRIX.md
docs/PHASE_3_FABRIC_SMOKE_TEST.md
docs/PHASE_3_FORGE_SMOKE_TEST.md
docs/PHASE_3_NEOFORGE_SMOKE_TEST.md
docs/ROADMAP.md
README.md
docs/adr/<component-graph>.md
docs/adr/<loader-registry>.md
docs/adr/<loader-processors>.md
docs/adr/<receipt-schema>.md        # when receipt schema changes
```

API/security docs must describe actual implementation, not copy this plan.

---

## 214. Definition of Done

Phase 3 is done when the following statement is demonstrably true:

> Starting with a supported exact Minecraft base version and an exact Fabric, Forge, or NeoForge
> loader release, a Graphene host can request a component-aware installation without knowing the
> loader's external metadata format. Graphene resolves the release through a registered provider,
> validates a deterministic component graph, composes provider-neutral Minecraft metadata, builds
> an inspectable install plan, acquires every remote input through the existing verified Artifact
> pipeline, performs required Forge/NeoForge preparation in bounded staging using an explicit Java
> tool runner, verifies generated outputs, transactionally commits the instance, persists exact
> component identity plus complete final launch metadata, then restarts offline and builds/executes
> the ordinary Phase 1 `LaunchPlan` with no Fabric/Forge/NeoForge branch in the launch engine.

Phase 3 is not done because:

- a loader main class was manually appended;
- `java -jar installer.jar` happened to work;
- one moving latest release worked once;
- service code contains provider-name branches everywhere;
- provider DTOs became persisted instance state;
- output hashes are ignored;
- processors write into committed state;
- old Vanilla receipts stop launching;
- tests depend on public moving services;
- crate roots become monolithic;
- fixtures are presented as real smoke passes.

---

## 215. Architectural North Star

```text
                           Graphene Facade
                                  |
                           Service Composition
                                  |
                     Loader Provider Registry
                  +---------------+---------------+
                  |               |               |
                Fabric          Forge          NeoForge
                  |               |               |
                  +------- normalized adapters ---+
                                  |
                                  v
                         Resolved Components
                                  |
                                  v
                          Component Graph
                                  |
                                  v
                       MinecraftVersionPatch
                                  |
                                  v
                         ResolvedMinecraft
                                  |
                +-----------------+-----------------+
                |                                   |
                v                                   v
          InstallPlan                        final launch metadata
                |
        +-------+--------+
        |                |
        v                v
 Artifact Pipeline   Preparation Recipe
        |                |
        |                v
        |          Java Tool Runner
        |                |
        |                v
        |         Generated Artifacts
        |                |
        +-------+--------+
                |
                v
       Transactional Commit
                |
                v
          InstallReceipt
                |
                v
        Existing LaunchPlan
                |
                v
             Process
```

The extension rule is:

> **A new loader should primarily add a provider adapter, normalized component/preparation data, and
> fixtures. If it requires authentication changes, a concrete Java-provider dependency, or
> loader-specific launch logic, the Phase 3 boundary must be re-examined before implementation
> proceeds.**
