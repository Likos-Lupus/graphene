# Graphene Phase 1 — Detailed Vanilla End-to-End Implementation Plan

**Status:** Approved implementation plan  
**Phase:** 1 — Vanilla End-to-End  
**Project version baseline:** 0.1.x  
**Primary implementation language:** Rust 2024  
**Implementation state:** Documentation only; this revision contains no Phase 1 Rust
implementation  
**Normative parent documents:** `PROJECT_SPECIFICATION.md`, `ARCHITECTURE.md`,
`SCOPE_AND_BOUNDARIES.md`, `ROADMAP.md`, `PHASE_0_IMPLEMENTATION_PLAN.md`,
`PHASE_0_API.md`, `PHASE_0_SECURITY_REVIEW.md`

---

## 1. Purpose

Phase 1 is Graphene's first complete product proof.

Phase 0 established the reusable foundation: explicit data roots, structured errors and diagnostics,
typed operations, progress, cancellation, platform normalization, network transport, artifact
acquisition, integrity verification, safe cache commit, and the root service facade.

Phase 1 must use those foundations to prove the complete **Vanilla install-to-launch vertical slice
** without violating the architectural boundaries established by the project specification.

The phase must turn this:

```text
Graphene foundation
```

into this:

```text
Official Minecraft metadata
        |
        v
Version discovery
        |
        v
Version metadata acquisition
        |
        v
Minecraft resolution
        |
        v
ResolvedMinecraft
        |
        v
InstallRequest
        |
        v
InstallPlan
        |
        v
Artifact acquisition
        |
        v
Transactional instance materialization
        |
        v
Installed Vanilla instance
        |
        v
Java discovery / probing / selection
        |
        v
LaunchRequest
        |
        v
LaunchPlan
        |
        v
Process spawn
        |
        v
Process output / exit lifecycle
```

Phase 1 is not an account, loader, content, modpack, repair, or UI phase.

---

## 2. Phase 1 Objective

The primary engineering objective is:

> Starting from a fresh Graphene data root, Graphene can discover an official Vanilla Minecraft
> version, resolve its complete launch/install model, calculate the entire installation plan before
> mutating committed instance state, acquire and verify all required artifacts through the Phase 0
> artifact pipeline, commit a create-only isolated instance transactionally, discover and select a
> compatible local Java runtime, generate a deterministic and inspectable `LaunchPlan`, spawn the
> Java process without a shell, stream process output and lifecycle events, and repeat installation
> work while reusing already verified shared artifacts.

The important product of Phase 1 is not merely "Minecraft starts." It is a set of stable, testable
engine contracts:

- `VersionManifest`;
- `MinecraftVersionMetadata`;
- `ResolvedMinecraft`;
- `InstallRequest`;
- `InstallPlan`;
- `InstalledInstance`;
- `JavaRuntime`;
- `JavaRequirement`;
- `JavaSelection`;
- `LaunchSession`;
- `LaunchRequest`;
- `LaunchPlan`;
- `RunningGame`.

These types and their boundaries must remain useful when Phase 2 adds accounts, Phase 3 adds
loaders, Phase 4 expands the instance engine, and later phases add content and modpacks.

---

## 3. Implementation Gate from Phase 0

Phase 1 source implementation must not begin on top of an unverified foundation.

Before the first Phase 1 code change is accepted, the Phase 0 baseline must satisfy all of the
following:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
python scripts/check_architecture.py
```

The Phase 0 artifact commit boundary must also satisfy its documented cancellation contract:
potentially blocking filesystem-lock acquisition must not occur after cancellation has been sealed
unless that wait is itself explicitly part of the non-interruptible commit boundary.

Phase 1 must preserve every Phase 0 invariant:

- no global mutable Graphene singleton;
- no UI framework dependency;
- no provider DTO leakage;
- bounded retries and network concurrency;
- streamed artifact downloads;
- integrity verification before cache commit;
- cancellable long operations;
- structured errors;
- no secret-bearing URLs in diagnostics/tracing;
- safe managed-path and cache boundaries.

If Phase 1 requires changing a Phase 0 contract, the change must be intentional, documented, tested,
and compatible with the architecture rather than introduced as an implementation shortcut.

---

## 4. Phase 1 Success Statement

Phase 1 is successful when all of the following are true:

1. An empty Graphene data root can be initialized.
2. Graphene can retrieve and parse the official Minecraft version manifest through a provider
   adapter.
3. A selected supported Vanilla version can be resolved into a provider-neutral
   `ResolvedMinecraft`.
4. The resolver handles the metadata features required by the supported version set, including:
   rules, arguments, libraries, assets, natives, logging metadata, Java requirements, and version
   inheritance where present.
5. `InstallPlan` contains every required immutable artifact and every final instance materialization
   action before the committed instance directory is created.
6. Installation uses Phase 0 acquisition/cache infrastructure rather than a second downloader.
7. Client, library, asset, native, logging, and metadata artifacts are verified whenever official
   metadata provides trustworthy integrity information.
8. A failed or cancelled installation never leaves a valid-looking committed instance.
9. Shared verified artifacts survive cancellation and can be reused safely.
10. A second installation of the same Vanilla version reuses valid cached/shared artifacts rather
    than redownloading them.
11. Graphene discovers local Java candidates without executing through a shell.
12. Java candidates are probed with bounded execution and normalized into `JavaRuntime`.
13. A compatible runtime is selected deterministically from `JavaRequirement`.
14. `LaunchPlan` can be generated without spawning Java.
15. `LaunchPlan` contains deterministic classpath, JVM arguments, main class, game arguments,
    working directory, environment delta, natives path, and Java executable.
16. Secret launch values are redacted from `Debug`, tracing, snapshots, and errors.
17. Process execution consumes argv directly and never constructs a shell command string.
18. stdout/stderr and process start/exit state are observable without a UI framework.
19. Deterministic CI tests do not depend on public Mojang servers or a preinstalled system Java.
20. A documented manual smoke procedure proves at least one real supported Vanilla release can be
    installed and launched through the library.

This is Graphene's first release-worthy technical milestone.

---

## 5. Scope

### 5.1 In Scope

Phase 1 includes:

1. Activation of the Minecraft domain context.
2. Activation of the Mojang provider adapter.
3. Official version-manifest retrieval.
4. Version selection by explicit version ID.
5. Normalized Minecraft version metadata.
6. Version metadata acquisition with integrity verification where a manifest digest is available.
7. Version inheritance resolution.
8. Mojang rule evaluation.
9. Modern `arguments.jvm` and `arguments.game`.
10. Legacy `minecraftArguments` lexical parsing for supported metadata.
11. Maven coordinate parsing and path calculation.
12. Library rule filtering.
13. Native classifier selection.
14. Asset-index acquisition and parsing.
15. Asset-object resolution.
16. Logging configuration acquisition.
17. Java requirement normalization from Minecraft metadata.
18. `ResolvedMinecraft`.
19. Minimal create-only instance identity/layout.
20. `InstallRequest`.
21. Deterministic `InstallPlan`.
22. Install-plan validation.
23. Transaction staging.
24. Shared library and asset materialization.
25. Client-JAR materialization.
26. Safe native extraction.
27. Minimal instance metadata.
28. Phase 1 install receipt.
29. Transaction validation and final commit.
30. Local Java discovery.
31. Java process probing.
32. Java version/vendor/architecture normalization.
33. Deterministic Java selection.
34. Externally supplied ephemeral `LaunchSession`.
35. `LaunchRequest`.
36. Launch placeholder substitution.
37. Classpath generation.
38. `LaunchPlan`.
39. Process spawn without a shell.
40. stdout/stderr draining.
41. Process lifecycle events and wait/kill handles.
42. Phase 1 service-facade expansion.
43. Frozen metadata fixtures.
44. Deterministic local HTTP fixtures.
45. Cross-platform fake-Java test support.
46. Plan snapshot tests.
47. Installation cancellation/failure tests.
48. Native archive security tests.
49. Cross-platform CI expansion.
50. Phase 1 API/security documentation after implementation matches the approved plan.

### 5.2 Explicitly Out of Scope

Phase 1 must not implement:

- Microsoft OAuth;
- account persistence;
- offline-account management;
- token refresh;
- secure persistent account secret storage;
- launcher-managed Java download/install;
- Fabric;
- NeoForge;
- Forge;
- Quilt;
- loader/component provider registries beyond interfaces needed for future extension;
- Modrinth;
- CurseForge;
- local mod management;
- modpacks;
- instance clone/delete/rename;
- global/per-instance configuration inheritance;
- mutable installed-instance updates;
- Graphene lockfile;
- repair planning;
- instance repair;
- content dependency resolution;
- rich crash diagnosis;
- launcher self-update;
- Tauri commands;
- Slint controllers;
- UI localization;
- dynamic runtime plugins;
- shell hooks;
- automatic server installation.

Phase 1 must not implement "temporary versions" of those features inside unrelated crates.

---

## 6. Supported-Version Policy

Phase 1 must avoid claiming complete historical Minecraft compatibility merely because metadata can
be parsed.

Three support levels are defined.

### 6.1 Tier A — Install and Launch Supported

A version is Tier A when Graphene can:

- obtain its metadata through the official manifest/provider path;
- normalize all required launch metadata;
- resolve every required library/native/asset/client/logging artifact;
- obtain safe local identity for artifacts;
- verify trustworthy declared hashes/sizes;
- determine a Java requirement;
- build a complete `InstallPlan`;
- materialize the instance;
- build a complete `LaunchPlan`.

Phase 1 end-to-end acceptance targets at least one pinned Tier A Vanilla release.

### 6.2 Tier B — Metadata/Resolution Supported

A version may be parseable and partially resolvable but not installable if it requires legacy
behavior Phase 1 intentionally does not yet support safely.

Examples may include metadata with:

- missing immutable artifact identity;
- legacy download conventions that cannot be verified through current integrity policy;
- unsupported architecture/native combinations;
- unsupported argument or asset behavior.

Tier B must return a structured support result rather than failing through an unrelated error.

### 6.3 Unsupported

Malformed or semantically unsupported metadata returns a stable structured error.

Tests must never use "latest" as the fixture identity. A pinned version ID and frozen metadata set
must be recorded so fixture behavior cannot change when Mojang publishes a new release.

---

## 7. Repository Transition

Phase 0 currently provides:

```text
graphene/
├── crates/
│   ├── graphene-core/
│   ├── graphene-platform/
│   ├── graphene-network/
│   ├── graphene-storage/
│   └── graphene-service/
└── src/lib.rs
```

Phase 1 activates the following bounded contexts:

```text
graphene/
├── crates/
│   ├── graphene-core/
│   ├── graphene-platform/
│   ├── graphene-network/
│   ├── graphene-storage/
│   ├── graphene-minecraft/
│   ├── graphene-instance/
│   ├── graphene-java/
│   ├── graphene-providers/
│   ├── graphene-install/
│   ├── graphene-launch/
│   └── graphene-service/
├── tests/
│   ├── fixtures/
│   │   ├── minecraft/
│   │   ├── artifacts/
│   │   ├── native-jars/
│   │   └── java/
│   └── integration/
└── docs/
```

No `graphene-auth`, `graphene-content`, `graphene-pack`, or `graphene-diagnostics` implementation is
activated merely to complete Phase 1.

---

## 8. Phase 1 Crate Responsibilities

### 8.1 `graphene-core`

Phase 1 may extend the stable foundation with:

- `InstanceId`;
- additional stable error codes;
- secret-redacted value wrappers where a launch value must exist transiently;
- provider-neutral acquisition/result primitives only when required to invert dependencies cleanly.

It must not gain:

- Minecraft metadata parsing;
- Java discovery;
- filesystem layout logic;
- provider URLs;
- launch argument rules.

### 8.2 `graphene-platform`

Phase 1 extends platform capabilities with process primitives required by Java probing and game
execution.

It owns:

- process command construction primitives;
- spawn;
- process ID;
- piped stdout/stderr access;
- bounded termination/kill primitives;
- platform classpath separator;
- executable-path checks;
- OS/architecture detection already established in Phase 0.

It must not decide:

- which Java version is compatible;
- which Minecraft main class to run;
- which JVM arguments are correct.

### 8.3 `graphene-network`

Phase 1 extends generic transport only where provider metadata requires it.

It may add:

- bounded metadata GET;
- maximum-body enforcement;
- provider-neutral response metadata;
- safe redirect/TLS behavior compatible with Phase 0;
- generic conditional-request support if implemented.

It must not expose `reqwest` types to provider/domain crates.

The top-level version manifest has no immutable hash supplied externally, so it must use a bounded
metadata request path rather than pretending it is a verified content-addressed artifact.

Version JSON and asset-index documents should use the verified artifact path whenever the parent
metadata supplies a digest.

### 8.4 `graphene-storage`

Phase 1 expands storage primitives for:

- managed instance staging directories;
- managed committed instance directories;
- shared libraries;
- shared assets;
- shared metadata;
- atomic materialization of verified immutable files;
- safe directory commit;
- relative-path validation;
- schema-versioned JSON writes.

Storage remains an implementation mechanism. It must not know how Minecraft rules or Java
requirements work.

### 8.5 `graphene-minecraft`

Owns the stable Minecraft domain and deterministic resolution logic:

- version IDs/types;
- normalized manifest;
- normalized version metadata;
- arguments;
- rules;
- rule context;
- Maven coordinates;
- libraries;
- native descriptors;
- assets;
- logging metadata;
- Java requirement declaration;
- version inheritance/merge;
- `ResolvedMinecraft`.

This crate must be usable in fixture tests without HTTP, filesystem mutation, Java execution, or UI.

### 8.6 `graphene-instance`

Phase 1 activates only the minimum create-only instance domain.

It owns:

- `InstanceId` usage;
- minimal instance descriptor;
- relative instance layout;
- schema-versioned `instance.json` domain representation;
- Phase 1 install-receipt domain representation;
- read-only installed-instance state required by launch planning.

Phase 1 instances are create-only committed snapshots.

This crate does not yet own:

- clone/delete/rename;
- mutable configuration inheritance;
- lockfile;
- repair;
- general-purpose instance mutation locks.

Those remain Phase 4 work.

### 8.7 `graphene-java`

Owns:

- `JavaRuntime`;
- `JavaRequirement`;
- candidate discovery policy;
- candidate deduplication;
- probe command definition;
- probe-output parsing;
- Java major-version parsing;
- architecture normalization;
- deterministic compatibility/selection.

It may use `graphene-platform` process/path primitives.

It must not:

- download Java;
- manage Java distributions;
- persist user Java preferences;
- show UI prompts.

### 8.8 `graphene-providers`

Phase 1 activates only the Mojang/Minecraft metadata adapter.

It owns:

- official manifest endpoint configuration;
- Mojang DTOs;
- JSON deserialization boundary;
- provider-specific URL conventions;
- conversion from Mojang DTOs into Graphene Minecraft domain types;
- conversion from Minecraft asset hashes into official asset sources;
- test endpoint injection.

No Mojang DTO may cross into `graphene-minecraft`, `graphene-install`, `graphene-launch`, the root
facade, or UI hosts.

### 8.9 `graphene-install`

Owns:

- `InstallRequest`;
- `InstallPlan`;
- plan validation;
- plan execution;
- staging;
- shared immutable materialization orchestration;
- native extraction orchestration;
- staged-instance validation;
- final create-only instance commit;
- cancellation semantics;
- install result.

It must not contain:

- raw Mojang HTTP;
- Mojang DTOs;
- Java discovery;
- launch argument generation;
- account behavior.

### 8.10 `graphene-launch`

Owns:

- `LaunchSession`;
- `LaunchRequest`;
- launch features;
- placeholder model;
- classpath construction;
- JVM/game argument expansion;
- `LaunchPlan`;
- redacted launch-plan debugging;
- mapping `LaunchPlan` to process execution;
- running-process lifecycle abstraction.

It must not authenticate a user or refresh a token.

### 8.11 `graphene-service`

Owns composition:

- default Mojang provider construction;
- Minecraft service wiring;
- install-service wiring;
- Java-service wiring;
- launch-service wiring;
- conversion between platform context and Minecraft rule context;
- adapters connecting Phase 0 artifact acquisition to Phase 1 installer ports.

It must not duplicate resolution/planning algorithms owned by the domain/use-case crates.

### 8.12 Root `graphene` Crate

The root facade expands from the Phase 0 foundation toward:

```text
graphene.minecraft()
graphene.instances()
graphene.java()
graphene.install()
graphene.launch()
```

The root crate may re-export Graphene-owned public domain types from their owning crates.

It must not become a second service implementation layer.

---

## 9. Required Dependency Direction

Phase 1 must remain acyclic.

Recommended direction:

```text
                           graphene
                              |
             +----------------+----------------+
             |                                 |
             v                                 v
      graphene-service                  public domain crates
             |
     +-------+---------+---------+---------+---------+
     |                 |         |         |         |
     v                 v         v         v         v
 providers          install    launch     java     instance
     |                 |         |         |
     v                 |         |         |
 minecraft <-----------+---------+         |
     |                   \                 /
     |                    \               /
     v                     v             v
   core                  storage      platform
                            \           /
                             \         /
                              v       v
                                core

network -> core
network -> platform only where required
providers -> network
providers -> minecraft
```

More concretely:

```text
graphene-core
    -> foundational libraries only

graphene-platform
    -> graphene-core

graphene-network
    -> graphene-core
    -> graphene-platform only where required

graphene-storage
    -> graphene-core
    -> graphene-platform

graphene-minecraft
    -> graphene-core

graphene-instance
    -> graphene-core

graphene-java
    -> graphene-core
    -> graphene-platform

graphene-providers
    -> graphene-core
    -> graphene-network
    -> graphene-minecraft

graphene-install
    -> graphene-core
    -> graphene-minecraft
    -> graphene-instance
    -> graphene-storage
    -> graphene-platform only for filesystem/extraction primitives when necessary

graphene-launch
    -> graphene-core
    -> graphene-minecraft
    -> graphene-instance
    -> graphene-java
    -> graphene-platform

graphene-service
    -> all Phase 1 implementation contexts

graphene
    -> graphene-core
    -> public Phase 1 domain crates
    -> graphene-service
```

### 9.1 Critical Dependency-Inversion Rule for Artifact Acquisition

The Phase 0 composite artifact acquisition currently lives behind the service layer.

`graphene-install` must **not** depend on `graphene-service`.

Phase 1 must therefore introduce an installer-owned or stable provider-neutral acquisition port,
conceptually:

```rust
pub trait ArtifactAcquirer {
    // Acquire one already-resolved artifact through the shared Graphene pipeline.
}
```

The concrete adapter is wired by `graphene-service` and delegates to the existing Phase 0 artifact
service.

The port result must contain only Graphene-owned values required by installation, for example:

```text
artifact identity
verified local path
verified byte size
verified digests
download/cache disposition
```

This preserves the one-way dependency graph and guarantees installation does not implement a second
network/cache stack.

### 9.2 Forbidden New Edges

Examples of architectural violations:

```text
graphene-minecraft -> graphene-network
graphene-minecraft -> graphene-platform
graphene-install   -> graphene-service
graphene-launch    -> graphene-service
graphene-launch    -> graphene-providers
graphene-java      -> graphene-service
graphene-instance  -> reqwest
graphene-providers -> graphene-service
any Phase 1 crate  -> tauri
any Phase 1 crate  -> slint
```

---

## 10. Minecraft Version Manifest Model

The provider adapter must normalize the official version manifest immediately.

Conceptual domain types:

```rust
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionSummary>,
}

pub struct LatestVersions {
    pub release: MinecraftVersionId,
    pub snapshot: MinecraftVersionId,
}

pub struct VersionSummary {
    pub id: MinecraftVersionId,
    pub version_type: MinecraftVersionType,
    pub metadata: MetadataArtifactRef,
    pub release_time: Option<String>,
    pub compliance_level: Option<u32>,
}
```

Provider-specific fields that are not required by the Graphene domain remain inside the provider
adapter.

### 10.1 Manifest Acquisition

Requirements:

- use the generic network layer;
- enforce a bounded maximum body size before JSON deserialization;
- accept only the configured official provider endpoint in production defaults;
- do not place credentials in URLs;
- use TLS in production defaults;
- allow local deterministic endpoint replacement in tests;
- reject malformed URLs/status/JSON through structured errors;
- preserve no raw provider response type outside `graphene-providers`.

The top manifest may be cached later using generic HTTP validators, but it must not be presented as
a content-addressed verified artifact when no external digest exists.

### 10.2 Explicit Version Selection

Phase 1 installs an explicit version ID.

The API may expose manifest listing helpers, but Phase 1 does not implement:

- search ranking;
- UI filtering;
- automatic "recommended" selection policy;
- silent replacement of an unknown requested version with latest.

An unknown version returns `MINECRAFT_VERSION_NOT_FOUND`.

---

## 11. Normalized Minecraft Version Metadata

The normalized model must represent all fields needed for deterministic resolution without leaking
Mojang DTOs.

Conceptually:

```text
MinecraftVersionMetadata
├── id
├── type
├── inherits_from?
├── main_class?
├── downloads
│   ├── client?
│   └── other provider-neutral entries as needed
├── arguments
│   ├── jvm
│   └── game
├── legacy_minecraft_arguments?
├── libraries[]
├── asset_index?
├── assets_id?
├── logging?
├── java_requirement?
├── release_time?
└── compliance metadata?
```

A provider adapter may deserialize a looser DTO and then validate/normalize it into this domain
model.

Missing fields that are legal for inherited metadata are allowed before inheritance resolution but
must be complete after `ResolvedMinecraft` validation for a Tier A version.

---

## 12. Version Metadata Acquisition

The manifest-supplied version metadata reference should be translated into the existing Graphene
artifact model whenever the official manifest provides a trustworthy digest.

Required behavior:

1. select the exact requested version;
2. create a metadata `Artifact`;
3. acquire it through Phase 0 artifact acquisition;
4. verify declared SHA-1/size where available;
5. deserialize through the provider boundary;
6. normalize into `MinecraftVersionMetadata`;
7. recursively acquire a parent only when `inheritsFrom` requires it;
8. detect cycles and excessive inheritance depth.

Metadata acquisition modifies only cache/shared metadata, not committed instance state.

---

## 13. Version Inheritance

Inheritance must be deterministic and explicit.

Required protections:

- cycle detection by version ID;
- maximum inheritance depth;
- duplicate-parent prevention;
- structured errors for missing parent metadata;
- deterministic merge tests.

The merge model should use a Graphene-owned `VersionPatch`/merge function rather than provider DTO
mutation.

### 13.1 Merge Direction

Parents are resolved first. Children apply later.

Conceptual flow:

```text
root parent
    |
    v
resolved parent
    |
    +-- apply child patch
            |
            v
      resolved child
```

### 13.2 Required Merge Categories

The implementation plan must define and test merge behavior for:

- main class;
- downloads;
- asset index;
- asset ID;
- logging;
- Java requirement;
- modern JVM arguments;
- modern game arguments;
- legacy game arguments;
- libraries;
- metadata flags required by launch.

For appendable lists such as arguments and libraries, parent order must be preserved and child
contributions must be applied deterministically.

Library replacement/deduplication must be based on a documented Maven-library identity rather than
string coincidence.

Any inheritance behavior added for future loaders must continue to produce the same normalized
`ResolvedMinecraft` contract rather than leaking loader-specific branches into launch logic.

---

## 14. Mojang Rule Model

Rules are domain logic and belong in `graphene-minecraft`.

Conceptual model:

```rust
pub enum RuleAction {
    Allow,
    Disallow,
}

pub struct Rule {
    pub action: RuleAction,
    pub os: Option<OsRule>,
    pub features: BTreeMap<String, bool>,
}

pub struct RuleContext {
    pub os: MinecraftOs,
    pub arch: MinecraftArch,
    pub os_version: Option<String>,
    pub features: BTreeMap<String, bool>,
}
```

### 14.1 Evaluation Semantics

Required deterministic behavior:

- no rules -> allowed;
- a non-empty rule set starts disallowed;
- evaluate rules in source order;
- when a rule matches its constraints, its action becomes the current result;
- non-matching rules do not change the result;
- the final matching action decides.

This handles patterns such as:

```text
allow all
disallow macOS
```

and:

```text
allow only when one feature is enabled
```

### 14.2 OS/Architecture Boundary

`graphene-minecraft` must not depend on `graphene-platform`.

The service layer converts platform information into Minecraft-domain rule values.

Minecraft OS identity should normalize at least:

```text
windows
linux
osx
```

Architecture handling must support the values needed by official metadata and native-classifier
substitution without assuming all unknown architectures are x86_64.

### 14.3 OS Version Patterns

If provider metadata contains OS version regular expressions:

- use a regex implementation with bounded/linear-time matching properties;
- cap accepted pattern length;
- return a metadata error for invalid patterns;
- never execute provider text as code or shell syntax.

### 14.4 Features

Rule features are a generic string-to-bool map so future Mojang features do not require an enum
breaking change.

Phase 1 launch planning must intentionally provide values for features it understands, including
custom-resolution state where applicable.

Unknown features default to `false` unless a future specification explicitly requires otherwise.

---

## 15. Argument Model

Graphene must preserve Minecraft arguments as argument vectors, never shell command strings.

Conceptually:

```rust
pub enum Argument {
    Literal(String),
    Conditional {
        rules: Vec<Rule>,
        values: Vec<String>,
    },
}
```

Resolution produces ordered argument templates after rule filtering.

### 15.1 Modern Arguments

Support:

```text
arguments.jvm
arguments.game
```

Each entry may be:

- a literal string;
- a rule-controlled object containing one value;
- a rule-controlled object containing multiple values.

Order must remain identical to normalized metadata order.

### 15.2 Legacy `minecraftArguments`

Legacy argument text must be tokenized by a dedicated deterministic lexical parser.

Requirements:

- parse into argv tokens;
- preserve quoted spaces;
- define escaping behavior in tests;
- never pass the raw string to a shell;
- reject malformed/unclosed quoting with a structured metadata error.

Legacy tokenization is parsing only. It does not imply shell compatibility.

---

## 16. Placeholder Model

Placeholder replacement belongs in launch planning, not metadata parsing.

At minimum Phase 1 must support the placeholders required by the Tier A fixture set, with the model
designed to cover common official values such as:

```text
${auth_player_name}
${version_name}
${game_directory}
${assets_root}
${assets_index_name}
${auth_uuid}
${auth_access_token}
${user_type}
${version_type}
${natives_directory}
${launcher_name}
${launcher_version}
${classpath}
${classpath_separator}
${library_directory}
${resolution_width}
${resolution_height}
${clientid}
${auth_xuid}
```

The implementation must distinguish:

- ordinary values;
- path values;
- secret values.

A selected argument referencing an unavailable required placeholder returns a structured
`LAUNCH_PLACEHOLDER_MISSING` error.

Arguments filtered out by rules must not require placeholders they never use.

---

## 17. Maven Coordinate Model

Maven parsing belongs in `graphene-minecraft`.

The model should represent:

```text
group
artifact
version
classifier?
extension
```

It must support the coordinate forms required by official Minecraft metadata.

Derived repository paths must be deterministic:

```text
group.with.dots
    ->
group/with/dots
```

Conceptually:

```text
<group path>/<artifact>/<version>/<artifact>-<version>[-classifier].<extension>
```

Requirements:

- reject path separators inside coordinate components;
- reject `..`;
- reject empty mandatory components;
- normalize extension safely;
- distinguish ordinary classpath artifacts from native classifiers;
- preserve official `downloads.*.path` when supplied and valid;
- use synthesized Maven paths only when the supported metadata rules explicitly permit it.

No raw Maven coordinate may become an unchecked filesystem path.

---

## 18. Library Resolution

Each library is resolved using:

1. version inheritance;
2. library-level rule evaluation;
3. platform/native classifier selection;
4. provider-supplied download descriptor;
5. Maven path validation;
6. integrity declaration.

A resolved library must state whether it contributes to:

- classpath;
- native extraction;
- neither because rules filtered it out.

Conceptual output:

```rust
pub struct ResolvedLibrary {
    pub coordinate: MavenCoordinate,
    pub classpath_artifact: Option<ResolvedArtifact>,
    pub native_artifact: Option<ResolvedArtifact>,
}
```

### 18.1 Classpath Rules

- preserve deterministic metadata order;
- remove only documented duplicates;
- never include a native classifier in the Java classpath unless metadata explicitly makes it an
  ordinary artifact;
- client JAR position is documented and tested;
- classpath separator comes from platform capability, not string guessing in the Minecraft crate.

### 18.2 Native Classifier Rules

Classifier selection must:

- use the current Minecraft rule OS/architecture context;
- expand only documented classifier placeholders;
- require a corresponding declared classifier download for Tier A support;
- fail structurally when the current platform has no supported native artifact.

---

## 19. Asset Resolution

Phase 1 must acquire the declared asset index before creating `InstallPlan`, because the complete
set of asset objects must be known before installation execution begins.

Conceptual model:

```rust
pub struct ResolvedAssets {
    pub index_id: String,
    pub index: ResolvedArtifact,
    pub objects: Vec<ResolvedAssetObject>,
    pub virtual_layout: bool,
    pub map_to_resources: bool,
}
```

Each asset object must contain:

```text
logical name
hash
size
resolved source
shared destination
```

### 19.1 Asset Object Identity

The content hash is authoritative for cache identity.

The provider adapter owns construction of the official asset source URL from the hash.

The Minecraft/install domain owns only the normalized artifact declaration and logical destination.

### 19.2 Shared Asset Layout

Phase 1 should materialize compatible launcher-style shared paths:

```text
shared/assets/
├── indexes/
│   └── <asset-index-id>.json
└── objects/
    └── <first-two-hash-chars>/
        └── <full-hash>
```

If the selected supported metadata requires virtual/resource mapping, the behavior must be
explicitly planned and tested before that version is Tier A.

### 19.3 Resource Bounds

Asset indexes can produce thousands of artifacts.

The executor must not:

- spawn one unbounded Tokio task per asset;
- retain unbounded output events;
- create an unbounded number of public retained operation objects.

Use bounded work scheduling and aggregate progress.

---

## 20. Client and Logging Artifacts

### 20.1 Client JAR

The resolved client JAR must contain:

- source(s);
- expected size where declared;
- trusted hash where declared;
- instance destination.

The client JAR is materialized only from verified acquisition output.

### 20.2 Logging Configuration

If client logging metadata exists:

- acquire the declared logging file;
- verify declared integrity;
- materialize it to a Graphene-managed stable location;
- preserve the logging JVM argument template in normalized launch metadata;
- substitute its local path during `LaunchPlan` generation.

Logging metadata must not create arbitrary filesystem paths.

---

## 21. `ResolvedMinecraft`

`ResolvedMinecraft` is the stable provider-neutral result of Minecraft resolution.

Conceptual direction:

```rust
pub struct ResolvedMinecraft {
    pub version_id: MinecraftVersionId,
    pub version_type: MinecraftVersionType,
    pub main_class: String,
    pub client: ResolvedArtifact,
    pub libraries: Vec<ResolvedLibrary>,
    pub assets: ResolvedAssets,
    pub logging: Option<ResolvedLogging>,
    pub jvm_args: Vec<Argument>,
    pub game_args: Vec<Argument>,
    pub java_requirement: JavaRequirement,
}
```

It must not contain:

- Mojang DTOs;
- `reqwest` types;
- UI types;
- absolute data-root paths;
- account persistence;
- process handles.

### 21.1 Resolution Invariants

A Tier A `ResolvedMinecraft` must guarantee:

- non-empty main class;
- client artifact present;
- asset model complete;
- all selected libraries internally valid;
- all selected native artifacts internally valid;
- launch arguments normalized;
- Java requirement available or intentionally represented by a documented fallback policy;
- no unresolved inheritance;
- no provider DTO dependency.

---

## 22. Phase 1 Instance Model

Phase 1 intentionally uses a **create-only installed instance** model.

This avoids prematurely implementing the mutable instance engine planned for Phase 4.

Conceptually:

```rust
pub struct InstanceDescriptor {
    pub id: InstanceId,
    pub name: String,
    pub schema_version: u32,
}

pub struct InstalledInstance {
    pub descriptor: InstanceDescriptor,
    pub minecraft: InstalledMinecraft,
}
```

### 22.1 Create-Only Rule

Phase 1 installation:

- generates or receives a typed `InstanceId`;
- stages a new directory;
- commits only if `instances/<id>` does not already exist;
- never mutates an existing committed instance in place.

Updating, repairing, replacing, cloning, renaming, and deleting are not Phase 1 operations.

### 22.2 Instance Layout

Target layout:

```text
instances/<instance-id>/
├── instance.json
├── .minecraft/
│   └── versions/
│       └── <version-id>/
│           ├── <version-id>.jar
│           └── <version-id>.json        # optional compatibility snapshot if planned
└── .graphene/
    ├── install.json
    └── natives/
        └── <resolved-native-set>/
```

Shared immutable data remains outside the instance:

```text
shared/
├── libraries/
├── assets/
└── metadata/
```

No symlink is required for a valid Phase 1 instance.

### 22.3 Relative Persistence

Persistent instance metadata must store managed relative references, hashes, IDs, and semantic data,
not absolute Graphene-root paths.

This allows the entire Graphene data root to move without invalidating instance metadata.

---

## 23. Phase 1 Install Receipt

Phase 1 needs a launchable committed description without prematurely introducing the Phase 4
Graphene lockfile.

Use a schema-versioned install receipt:

```text
instances/<id>/.graphene/install.json
```

The receipt conceptually records:

```text
schema version
instance ID
requested Minecraft version
resolved Minecraft version
provider-neutral main class
client relative path + integrity
classpath library relative paths + integrity
asset index ID
asset root reference
logging configuration reference
native directory reference
normalized JVM/game argument templates
Java requirement
installation format/version
```

The receipt is:

- authoritative for Phase 1 launch reconstruction;
- generated from `InstallPlan`;
- validated before commit;
- provider-neutral;
- free of access tokens;
- free of absolute data-root paths.

It is **not** the final Graphene lockfile.

Phase 4 may migrate or supersede its responsibilities when mutable instance verification/repair is
introduced.

---

## 24. `InstallRequest`

Conceptual API:

```rust
pub struct InstallRequest {
    pub instance: NewInstanceSpec,
    pub minecraft_version: MinecraftVersionId,
}
```

`NewInstanceSpec` should contain only Phase 1 creation information such as:

```text
InstanceId
display name
```

It must not grow Phase 4 configuration or Phase 5 content fields prematurely.

### 24.1 Request Validation

Before network work:

- instance ID must be valid;
- display name must satisfy bounded metadata rules;
- requested version ID must be non-empty and bounded;
- final managed target must be derivable safely;
- an already committed target produces `INSTALL_TARGET_EXISTS`.

---

## 25. Planning API

Plan generation is a first-class operation because it may require metadata and asset-index
acquisition.

Preferred shape:

```rust
let prepared = graphene.install().plan(request);
let operation = prepared.operation();
let plan = prepared.await_result().await?;
```

`InstallPlan` must be inspectable without executing installation.

Plan creation may populate verified caches for metadata documents but must not create the final
instance directory.

---

## 26. `InstallPlan`

A plan must fully describe intended state before instance commit.

Conceptually:

```rust
pub struct InstallPlan {
    pub plan_version: u32,
    pub instance: PlannedInstance,
    pub minecraft: ResolvedMinecraft,
    pub artifacts: Vec<PlannedArtifact>,
    pub shared_materializations: Vec<Materialization>,
    pub instance_materializations: Vec<Materialization>,
    pub native_extractions: Vec<NativeExtraction>,
    pub receipt: PlannedInstallReceipt,
}
```

### 26.1 Plan Properties

The plan must be:

- deterministic for the same normalized inputs;
- serializable for safe debugging where useful;
- snapshot-testable after normalization of random IDs/temp roots;
- independent of provider DTOs;
- independent of live HTTP clients;
- validated before execution;
- explicit about every final destination.

### 26.2 Plan Validation

Validation must reject:

- duplicate conflicting destinations;
- absolute managed-relative paths;
- `..` traversal;
- artifact without a usable source;
- missing mandatory client;
- incomplete assets;
- unsupported native platform;
- unresolved argument/inheritance state;
- materialization outside Graphene-managed roots;
- mismatched install receipt references;
- duplicate instance target.

---

## 27. Installation Execution Pipeline

Execution must follow a predictable stage machine.

Recommended stages:

```text
Validate Plan
     |
     v
Prepare Operation
     |
     v
Acquire Client
     |
     v
Acquire Libraries
     |
     v
Acquire Asset Index / Assets
     |
     v
Acquire Logging / Native JARs
     |
     v
Materialize Shared Immutable Files
     |
     v
Create Instance Staging Directory
     |
     v
Materialize Client / Instance Files
     |
     v
Extract Natives
     |
     v
Write instance.json
     |
     v
Write install.json
     |
     v
Validate Staged Instance
     |
     v
Final Cancellation Check
     |
     v
Commit New Instance Directory
     |
     v
Succeeded
```

### 27.1 Reuse Phase 0 Artifact Pipeline

Every remote immutable artifact must pass through the Phase 0 acquisition path.

No Phase 1 crate may directly:

- use `reqwest` for artifact downloads;
- implement independent retry loops;
- implement independent cache identity;
- bypass hash verification;
- stream directly into committed instance files.

### 27.2 Shared Immutable Materialization

Libraries/assets/shared metadata may be materialized before instance commit because they are
immutable, verified, shared resources rather than committed per-instance state.

Required behavior:

- existing valid destination -> reuse;
- missing destination -> materialize from verified cache output;
- conflicting/invalid destination -> safely replace only according to storage policy;
- prefer hardlink where safe and supported;
- fall back to copy;
- always use temporary destination + safe rename/replace rather than copy into a visible final file;
- never trust destination name as proof of integrity.

A cancelled install may leave valid shared immutable artifacts. That is expected and useful.

### 27.3 Instance Staging

Use a Graphene-managed sibling staging area conceptually like:

```text
instances/.graphene-staging-<operation-id>/
```

The staging path must not be confused with a committed instance.

All instance-private materialization happens in staging.

### 27.4 Final Commit

Phase 1 commits only a **new** instance.

The point of no return is the final safe directory publication.

Cancellation must remain possible while waiting for any potentially blocking pre-commit lock or
filesystem prerequisite.

Only immediately before the actual non-interruptible final publication may cancellation be sealed.

A cancellation request that wins before the seal produces `Cancelled`.

A late cancellation after successful commit must not relabel success.

---

## 28. Installation Progress Model

One install can involve thousands of assets, so progress must be hierarchical and bounded.

Recommended shape:

```text
Install Vanilla
├── Resolve metadata
├── Plan artifacts
├── Download
│   ├── Client
│   ├── Libraries
│   ├── Assets
│   ├── Natives
│   └── Logging
├── Materialize shared files
├── Stage instance
├── Extract natives
├── Validate
└── Commit
```

### 28.1 Aggregate Progress

Use:

- item counts when sizes are unavailable;
- byte totals when trustworthy sizes are known;
- stage-level progress;
- coalesced progress updates.

Do not publish one permanently retained public operation for every asset object.

If the Phase 0 artifact service currently requires one registered operation per artifact, Phase 1
must introduce an internal/aggregate acquisition path or bounded terminal-operation retention before
large asset fanout is enabled.

### 28.2 Cancellation

Cancellation checkpoints are required:

- before metadata requests;
- while waiting for artifact concurrency;
- between artifact batches;
- during retry delays;
- during large shared-file materialization;
- during native extraction;
- before staged validation;
- before final commit.

Large blocking filesystem/archive work must run outside async I/O worker threads and still expose
cooperative cancellation where practical.

---

## 29. Native Extraction

Native JARs are executable-code containers and must be handled as untrusted archives even when
officially hashed.

Required archive protections:

- verify the native JAR before extraction;
- reject absolute entry paths;
- reject `..` traversal;
- reject target escape after normalization;
- reject or ignore symbolic-link-like entries according to a documented safe policy;
- extract regular files only;
- apply provider metadata exclusions;
- exclude `META-INF/` when required by metadata;
- cap archive entry count;
- cap individual uncompressed entry size;
- cap total extracted bytes;
- reject implausible expansion ratios where appropriate;
- use a staging directory;
- do not overwrite outside the native staging root;
- run blocking archive work through a blocking-task boundary.

Native extraction must never invoke external archive tools or a shell.

---

## 30. Staged Instance Validation

Before commit, validation must prove at minimum:

- `instance.json` exists and round-trips;
- `.graphene/install.json` exists and round-trips;
- instance ID in metadata matches target ID;
- requested/resolved version identity is consistent;
- client JAR exists;
- all classpath library references exist;
- asset-index reference exists;
- required logging file exists when configured;
- native directory exists when natives are required;
- receipt relative paths resolve inside approved roots;
- no temporary `.part` file is referenced;
- no staging-only absolute path is persisted;
- `ResolvedMinecraft`/receipt has a valid main class and Java requirement for Tier A.

Commit must not occur if staged validation fails.

---

## 31. Java Requirement Model

Phase 1 normalizes Minecraft metadata into:

```rust
pub struct JavaRequirement {
    pub major_version: u32,
    pub component_hint: Option<String>,
}
```

`component_hint` is metadata, not a mandate to download a managed runtime in Phase 1.

If official metadata lacks a Java requirement for a supported historical version, any fallback
policy must be:

- explicit;
- versioned;
- fixture-tested;
- represented in the resolved model;
- not inferred from UI heuristics.

---

## 32. Java Discovery

Phase 1 discovers local Java only.

Candidate sources should include, where applicable:

```text
explicit request override
JAVA_HOME
PATH
common OS installation roots
platform-specific Java discovery mechanisms
```

Examples of platform search roots may include:

```text
Windows:
  JAVA_HOME
  PATH
  recognized Program Files Java/JDK roots

Linux:
  JAVA_HOME
  PATH
  /usr/lib/jvm/*/bin/java

macOS:
  JAVA_HOME
  PATH
  /Library/Java/JavaVirtualMachines/*/Contents/Home/bin/java
  platform Java-home discovery where safely available
```

Exact discovery adapters belong in `graphene-java` using `graphene-platform`.

### 32.1 Candidate Rules

- normalize and deduplicate paths;
- canonicalize only where safe and useful;
- do not follow arbitrary user-controlled recursive directory trees;
- cap the number of candidates;
- do not execute through a shell;
- treat discovery failure of one candidate as local evidence, not a global crash.

---

## 33. Java Probing

A Java candidate must be probed by executing the binary directly with a controlled argument set.

The probe must recover, where possible:

```text
Java version
major version
vendor
runtime/home path
reported architecture
```

### 33.1 Probe Safety

- direct argv process spawn;
- no shell;
- short timeout;
- bounded stdout/stderr capture;
- terminate hung probe;
- reject absurd output size;
- do not trust filename as version;
- structured per-candidate failure;
- no arbitrary environment mutation.

### 33.2 Java Version Parsing

At minimum support:

```text
1.8.x   -> 8
8       -> 8
17.x    -> 17
21.x    -> 21
future integer-major formats
```

The parser must have focused unit tests.

---

## 34. Java Selection

Selection must be deterministic.

Conceptual priority:

1. explicit launch/request override if it probes successfully and is compatible;
2. compatible runtime from normalized discovery candidates;
3. deterministic tie-breaking.

Compatibility must consider:

- required major version;
- executable validity;
- architecture compatibility with the current launch target.

Vendor preference must not be hard-coded unless an explicit project policy is documented later.

When no compatible Java exists, return `JAVA_NOT_FOUND` or `JAVA_INCOMPATIBLE` with non-secret
structured context such as:

```text
required major
candidate count
observed major versions
current architecture
```

Managed Java installation remains Phase 2.

---

## 35. Launch Session Boundary

Phase 1 must launch without implementing account management.

Therefore `graphene-launch` accepts an **ephemeral externally supplied** session object.

Conceptually:

```rust
pub struct LaunchSession {
    pub username: String,
    pub uuid: String,
    pub access_token: SensitiveString,
    pub user_type: String,
    pub client_id: Option<SensitiveString>,
    pub xuid: Option<SensitiveString>,
}
```

The exact final fields should follow the placeholders required by the supported Minecraft metadata.

### 35.1 Important Boundary

Phase 1 does **not**:

- authenticate this session;
- refresh it;
- persist it;
- validate Minecraft ownership;
- model a saved account.

Phase 2 account services should later produce the same launch-session contract.

This keeps launch planning independent from the authentication provider.

### 35.2 Secret Contract

Secret-bearing fields:

- redact `Debug`;
- redact error context;
- redact tracing;
- redact snapshot output;
- never persist to `instance.json` or `install.json`;
- only become real argv values immediately before process spawn.

---

## 36. `LaunchRequest`

Conceptual shape:

```rust
pub struct LaunchRequest {
    pub instance_id: InstanceId,
    pub session: LaunchSession,
    pub java_override: Option<PathBuf>,
    pub resolution: Option<LaunchResolution>,
    pub extra_jvm_args: Vec<String>,
    pub extra_game_args: Vec<String>,
}
```

Phase 1 should be conservative with user overrides.

Arbitrary extra arguments may be supported only as argv elements, never as a shell string.

Memory configuration does not require a Phase 4 configuration system. If Phase 1 exposes JVM memory
overrides, they belong to this explicit request and do not become persisted global/instance
configuration yet.

---

## 37. Launch Planning Must Be Offline

Once a Phase 1 instance is committed, `LaunchPlan` generation must not require Mojang metadata
network access.

Launch planning reads:

- committed instance descriptor;
- committed Phase 1 install receipt;
- shared/instance filesystem references;
- local Java discovery/probe result;
- supplied launch session;
- local platform context.

This ensures a previously installed instance can be planned offline.

Authentication validity is not a Phase 1 responsibility.

---

## 38. `LaunchPlan`

Conceptual direction:

```rust
pub struct LaunchPlan {
    pub instance_id: InstanceId,
    pub java: JavaRuntime,
    pub working_directory: PathBuf,
    pub environment: EnvironmentDelta,
    pub jvm_args: Vec<LaunchArgument>,
    pub classpath: Vec<PathBuf>,
    pub main_class: String,
    pub game_args: Vec<LaunchArgument>,
    pub natives_directory: PathBuf,
}
```

`LaunchArgument` should preserve secret classification:

```rust
pub enum LaunchArgument {
    Plain(String),
    Secret(SensitiveString),
}
```

The executable argv is materialized only for process spawn.

### 38.1 LaunchPlan Invariants

- Java path is explicit;
- working directory is the instance game directory;
- main class is non-empty;
- classpath entries exist or a clear validation error is returned;
- classpath order is deterministic;
- platform separator is explicit;
- all selected placeholders are resolved;
- no raw `${...}` placeholder remains in Tier A plan output;
- secrets are classified;
- no shell syntax is required;
- plan debugging is redacted.

### 38.2 Snapshot Representation

Provide a redacted normalized representation for tests/debugging:

```text
java = <normalized-path>
jvm args = [...]
classpath = [<normalized-root>/...]
main class = ...
game args = [..., <secret>, ...]
```

Random IDs, temp roots, and secret values must not make snapshots unstable.

---

## 39. Classpath Construction

Classpath construction belongs in `graphene-launch`.

Required rules:

- use only resolved classpath libraries;
- preserve deterministic library order;
- append/place the client JAR according to documented launcher semantics;
- remove exact duplicate paths only when doing so cannot change behavior;
- use the platform-provided classpath separator at argv materialization time;
- never quote the entire classpath as a shell string;
- do not include native-only JARs unless they are also ordinary classpath dependencies.

The plan should retain classpath as `Vec<PathBuf>` for inspectability and convert it to one Java
`-cp` argument only at the final plan/materialization boundary.

---

## 40. JVM Argument Resolution

The final JVM argument sequence is constructed from:

1. resolved official JVM argument templates;
2. rule filtering using the current rule context;
3. placeholder substitution;
4. explicit Phase 1 caller overrides, if supported by policy.

The implementation must not silently reorder official arguments.

Potential duplicate system properties must follow a documented order so override behavior is
predictable.

No argument may be passed through `cmd.exe`, `/bin/sh`, PowerShell, or another shell.

---

## 41. Game Argument Resolution

Game arguments follow the same vector-first model.

Required sequence:

1. choose modern or legacy normalized argument source;
2. evaluate rules;
3. expand values;
4. substitute launch placeholders;
5. preserve secret classification;
6. append explicitly allowed caller arguments if the API permits them.

Custom-resolution feature flags must be derived from `LaunchRequest`.

Unknown feature-gated arguments remain filtered unless their feature becomes explicitly supported.

---

## 42. Process Execution

`graphene-launch` owns game-process lifecycle semantics while `graphene-platform` owns OS process
primitives.

Execution flow:

```text
LaunchPlan
    |
    v
Validate executable/paths
    |
    v
Materialize argv
    |
    v
Spawn Java directly
    |
    +--> stdout drain
    |
    +--> stderr drain
    |
    +--> exit wait
    |
    v
RunningGame
```

### 42.1 No Shell

Required:

```rust
Command::new(java)
.arg(...)
.arg(...)
```

Forbidden:

```text
"java ... all args ..." -> shell
```

### 42.2 `RunningGame`

Conceptually provides:

```text
process ID
output subscription
wait()
kill()
current lifecycle snapshot
```

Graceful Minecraft-specific shutdown, play-time accounting, and crash diagnosis may be expanded
later.

### 42.3 Output Draining

stdout and stderr must be drained concurrently so a full pipe cannot deadlock the child.

Use bounded host-facing event queues.

If the consumer is too slow:

- continue draining the OS pipe;
- use an explicit bounded drop/coalescing policy;
- expose dropped-output count/state where appropriate;
- never block the game solely because a UI is not reading events quickly enough.

### 42.4 Output Encoding

Minecraft/Java output may not always be valid UTF-8.

The process layer must define a deterministic policy:

- preserve raw bytes internally where practical;
- provide lossy line conversion for host-friendly events;
- cap maximum buffered line/chunk size;
- never panic on invalid encoding.

---

## 43. Process Lifecycle Events

Recommended Graphene-owned lifecycle:

```text
Starting
Started { pid }
Stdout { ... }
Stderr { ... }
Exited { status }
Failed { code }
```

A failed spawn returns a structured error and never creates a fake running handle.

A non-zero game exit is a real exit state; whether it is returned as an error or an exit result must
be documented consistently.

Phase 1 diagnostics do not attempt to interpret a non-zero exit as a specific mod/crash cause.

---

## 44. Service-Facade Direction

The root experience should move toward:

```rust
let graphene = Graphene::builder(data_root)
.build()
.await?;

let versions = graphene
.minecraft()
.versions()
.await?;

let prepared = graphene
.install()
.plan(
InstallRequest::new("Vanilla Test", "pinned-version-id")
);

let plan = prepared.await_result().await?;

let installed = graphene
.install()
.execute(plan)
.await_result()
.await?;

let java = graphene
.java()
.select_for_instance(installed.id(), None)
.await?;

let launch_plan = graphene
.launch()
.plan(
LaunchRequest::new(installed.id(), external_session)
)
.await?;

let running = graphene
.launch()
.execute(launch_plan)
.await?;

let exit = running.wait().await?;
```

This is conceptual API direction, not a requirement to copy exact method names.

The important properties are:

- plan and execute remain separable;
- host code does not parse Mojang JSON;
- host code does not build classpaths;
- host code does not select natives;
- host code does not implement Java probing;
- host code does not construct shell commands.

---

## 45. Provider Configuration and Testability

Production Graphene should construct the official Mojang provider by default.

Tests require deterministic endpoint injection.

The provider must therefore have an internal/testable configuration containing at least the manifest
endpoint and any provider URL bases that cannot be derived from normalized metadata.

Do not add a broad public "arbitrary provider plugin" API in Phase 1.

Test injection may be exposed through:

- crate-internal constructors;
- integration-test support;
- a narrowly scoped provider configuration object.

The production default remains explicit and stable.

---

## 46. Metadata and JSON Safety

Remote metadata is untrusted input.

Every provider JSON path must have:

- response byte limits;
- timeout inherited from network policy;
- bounded recursion/data expectations;
- semantic validation after deserialization;
- bounded string fields where used as identifiers/paths;
- structured parse/validation errors;
- no panic on unknown enum/string values where forward-compatible handling is possible.

Unknown fields should normally be ignored at the DTO boundary.

Unknown required semantic values must produce an explicit unsupported/invalid result rather than
being guessed.

---

## 47. URL and Transport Safety

Official provider defaults must:

- use HTTPS;
- preserve TLS certificate validation;
- use bounded redirect policy;
- reject unsafe redirect downgrades according to network policy;
- avoid userinfo credentials in remote artifact URLs;
- validate provider-derived schemes;
- keep full artifact URLs out of logs/errors where they may contain sensitive query data.

Local HTTP is allowed only for deterministic fixtures or explicitly non-production test
configuration.

Phase 1 must not weaken Phase 0 network defaults to accommodate tests.

---

## 48. Filesystem Security

All provider-controlled names must be converted to validated managed-relative paths before
filesystem use.

Reject:

```text
absolute paths
parent traversal
empty required path components
platform prefix escape
UNC/drive escape where not explicitly managed
NUL-like invalid path data
```

Before final materialization, every destination must be proven to reside under one of:

```text
shared/libraries
shared/assets
shared/metadata
instances/<id> staging
```

Provider text must never choose an arbitrary host path.

---

## 49. Secret and Logging Security

Phase 1 introduces the first secret-bearing runtime value: the externally supplied access token.

Security rules:

- `Debug` redacts secret values;
- `Display` does not reveal secret values;
- tracing does not record them;
- `LaunchPlan` snapshot/debug export redacts them;
- errors do not include full argv when argv contains secrets;
- process-spawn failures report safe argument counts/categories rather than complete secret argv;
- instance/install metadata never persists them.

A dedicated redacted launch-plan representation is preferred over ad-hoc string replacement.

---

## 50. Performance and Resource Policy

Phase 1 must be designed for Minecraft-scale artifact counts.

### 50.1 Metadata

- bounded response bodies;
- parse one version/asset index at a time;
- avoid retaining raw provider JSON after normalization unless required for a documented cache.

### 50.2 Artifacts

- bounded in-flight acquisition;
- shared cache deduplication;
- aggregate progress;
- no unbounded task creation.

### 50.3 Filesystem

- large copy/hash/extraction work uses blocking-task boundaries;
- prefer hardlink for already verified immutable shared objects where safe;
- copy fallback must be streamed/bounded;
- final visibility only after safe materialization.

### 50.4 Process Output

- bounded host event channel;
- concurrent pipe drains;
- maximum line/chunk size.

---

## 51. Error Contract Expansion

Phase 1 should extend stable error codes rather than collapse failures into strings.

Recommended categories include:

```text
MINECRAFT_MANIFEST_INVALID
MINECRAFT_VERSION_NOT_FOUND
MINECRAFT_METADATA_INVALID
MINECRAFT_METADATA_UNSUPPORTED
MINECRAFT_INHERITANCE_CYCLE
MINECRAFT_INHERITANCE_TOO_DEEP
MINECRAFT_RULE_INVALID
MINECRAFT_LIBRARY_INVALID
MINECRAFT_ASSET_INDEX_INVALID
MINECRAFT_NATIVE_UNAVAILABLE
MINECRAFT_ARGUMENT_INVALID

INSTALL_REQUEST_INVALID
INSTALL_PLAN_INVALID
INSTALL_TARGET_EXISTS
INSTALL_STAGE_FAILED
INSTALL_NATIVE_EXTRACTION_FAILED
INSTALL_VALIDATION_FAILED
INSTALL_COMMIT_FAILED
INSTALL_CANCELLED

JAVA_NOT_FOUND
JAVA_PROBE_FAILED
JAVA_PROBE_TIMEOUT
JAVA_INCOMPATIBLE

LAUNCH_INSTANCE_INVALID
LAUNCH_SESSION_INVALID
LAUNCH_PLACEHOLDER_MISSING
LAUNCH_PLAN_INVALID
LAUNCH_PROCESS_SPAWN_FAILED
LAUNCH_PROCESS_IO_FAILED
```

Exact enum spelling may follow existing Rust naming conventions, while `as_str()` remains stable
machine-readable SCREAMING_SNAKE_CASE.

### 51.1 Error Context

Safe context may include:

```text
version ID
instance ID
artifact ID
library coordinate
asset logical name or hash prefix
required Java major
observed Java majors
stage
process exit code
```

Do not include:

```text
access token
client secret
full secret argv
credential-bearing URL
arbitrary complete environment
```

---

## 52. Diagnostics Boundary

Phase 1 may emit simple structured evidence through the existing diagnostic foundation, but it must
not expand into the rich diagnostics phase.

Examples of acceptable Phase 1 evidence:

```text
JAVA_REQUIREMENT_UNSATISFIED
MISSING_MATERIALIZED_LIBRARY
UNSUPPORTED_NATIVE_PLATFORM
```

Examples intentionally deferred:

- crash-cause ranking;
- mod conflict analysis;
- automatic repair recommendations;
- rich log interpretation.

---

## 53. Deterministic Fixture Strategy

CI must not depend on the public Minecraft infrastructure.

Required frozen fixtures:

```text
tests/fixtures/minecraft/
├── manifest.json
├── versions/
│   ├── modern/
│   │   └── version.json
│   ├── inheritance/
│   │   ├── parent.json
│   │   └── child.json
│   └── legacy-arguments/
│       └── version.json
├── assets/
│   └── index.json
└── README.md

tests/fixtures/artifacts/
├── client.jar
├── libraries/
├── assets/
├── logging/
└── hashes.json

tests/fixtures/native-jars/
├── valid.jar
├── traversal.jar
├── absolute-path.jar
├── oversized.jar
└── symlink-like.jar
```

Fixtures should model official metadata shapes while remaining deterministic and small.

The fixture README records:

- origin/shape purpose;
- whether content was generated or frozen from public metadata;
- expected digests;
- which behavior each fixture covers.

---

## 54. Fake Java Test Support

CI must not require Java to be preinstalled.

Phase 1 should include a cross-platform test helper executable written in Rust.

The helper can emulate:

1. Java probe output;
2. launch invocation;
3. stdout/stderr emission;
4. configurable exit code;
5. delayed output;
6. hung process for timeout/kill tests.

This is preferable to shell scripts because:

- it works on Windows/Linux/macOS;
- it does not test through a shell;
- it can expose deterministic argv/environment observations.

The helper is test support only and must not become production Java behavior.

---

## 55. Unit Test Matrix

### 55.1 Minecraft

Required unit tests include:

- manifest parsing/normalization;
- unknown version;
- version-type normalization;
- inheritance parent-first merge;
- inheritance cycle;
- inheritance maximum depth;
- scalar child override;
- argument append/order;
- library merge identity;
- no-rules allowed;
- allow-only rule;
- allow-then-disallow rule;
- OS match/non-match;
- architecture match/non-match;
- feature match/non-match;
- invalid OS regex;
- Maven coordinate parsing;
- invalid Maven path component;
- classpath artifact selection;
- native classifier selection;
- missing native classifier;
- asset hash/path derivation;
- modern arguments;
- legacy quoted argument parsing;
- malformed legacy quoting.

### 55.2 Java

Required unit tests:

- `1.8` -> major 8;
- integer-major Java versions;
- vendor normalization;
- architecture normalization;
- duplicate candidate elimination;
- explicit override priority;
- compatible selection;
- incompatible major;
- incompatible architecture;
- deterministic tie-breaking;
- malformed probe output.

### 55.3 Launch

Required unit tests:

- placeholder expansion;
- missing placeholder;
- secret placeholder classification;
- rule-filtered missing placeholder does not fail;
- classpath ordering;
- platform separator;
- working-directory construction;
- JVM argument order;
- game argument order;
- redacted Debug/snapshot representation.

---

## 56. Integration Test Matrix

All normal integration tests use a local deterministic HTTP fixture server.

### 56.1 Metadata/Resolution

```text
local manifest
    ->
verified version JSON
    ->
verified asset index
    ->
ResolvedMinecraft
```

Assert:

- no provider DTO leaks;
- expected libraries/assets/natives;
- expected Java requirement;
- stable resolution snapshot.

### 56.2 Complete Install

```text
fresh root
    ->
plan pinned fixture version
    ->
execute
    ->
committed instance
```

Assert:

- final instance appears only after success;
- install receipt round-trips;
- all paths remain inside managed roots;
- shared libraries/assets exist;
- client exists;
- natives extracted safely.

### 56.3 Cache Reuse

Install the same fixture version into a second create-only instance.

Assert:

- remote artifact request counts do not increase for already verified immutable artifacts where
  cache/shared reuse applies;
- no corrupt replacement occurs;
- second instance remains independent.

### 56.4 Cancellation

Test cancellation:

- during metadata request;
- while waiting for download capacity;
- during large asset transfer;
- during artifact batch;
- during native extraction;
- immediately before instance commit.

Assert:

- no committed instance exists;
- staging is cleaned;
- valid shared cache objects may remain;
- terminal operation state is `Cancelled`.

### 56.5 Failure

Inject:

- malformed manifest;
- malformed version JSON;
- hash mismatch;
- asset-index mismatch;
- one missing library;
- native traversal archive;
- disk/materialization failure;
- target collision;
- staged validation failure.

Assert structured error codes and no valid-looking committed instance.

### 56.6 Java/Launch

Use fake Java to:

- discover/probe a compatible runtime;
- build a launch plan;
- inspect received argv;
- verify secret value reaches the child but never debug output;
- stream stdout/stderr;
- observe PID;
- return exit 0;
- return non-zero exit;
- exercise kill/wait.

---

## 57. Snapshot Tests

Snapshot-test normalized versions of:

```text
ResolvedMinecraft
InstallPlan
install receipt
LaunchPlan (redacted)
```

Normalization must replace:

- temporary root;
- random operation IDs;
- random instance IDs when the test does not fix them;
- secret values.

Snapshots should reveal semantic regressions in:

- library order;
- arguments;
- assets;
- Java requirement;
- classpath;
- destinations.

---

## 58. Property and Fuzz Testing

Phase 1 should add focused property/fuzz targets where parser risk is meaningful.

High-value targets:

- Maven coordinates;
- legacy argument tokenizer;
- path normalization;
- provider JSON normalization;
- native archive entry names.

Fuzzing must not execute arbitrary Java or shell commands.

Archive fuzz targets should operate on bounded input sizes.

---

## 59. Architecture Tests

`scripts/check_architecture.py` and/or equivalent tests must be updated to reject:

```text
graphene-minecraft -> graphene-network
graphene-minecraft -> graphene-platform
graphene-install   -> graphene-service
graphene-launch    -> graphene-service
graphene-launch    -> graphene-providers
graphene-providers -> graphene-service
domain crates      -> reqwest
backend crates     -> tauri/slint
```

Also assert:

- `reqwest` remains confined to the network implementation;
- provider DTO modules are not publicly re-exported;
- root facade does not contain direct provider parsing.

---

## 60. Cross-Platform CI

Phase 1 CI should continue Windows/Linux/macOS coverage.

Required commands:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
python scripts/check_architecture.py
```

Where feasible also exercise:

- fake Java process tests on all three OSes;
- classpath separator snapshots per OS;
- native extraction tests;
- managed-path tests.

CI must not contact public Mojang endpoints.

---

## 61. Real Vanilla Smoke Test

Deterministic CI proves behavior without the public internet, but Phase 1's milestone also requires
a real Vanilla proof.

Before final Phase 1 sign-off, run a documented manual/opt-in smoke procedure:

1. start with a new Graphene data root;
2. select one explicitly pinned Tier A official Vanilla version;
3. retrieve official manifest/metadata;
4. produce and inspect `InstallPlan`;
5. execute installation;
6. verify cache/materialized artifacts;
7. discover or explicitly provide a compatible local Java;
8. supply an externally obtained ephemeral launch session if the selected client requires it;
9. build and inspect redacted `LaunchPlan`;
10. launch through Graphene;
11. observe Java/Minecraft process start and output;
12. close/kill the process cleanly for the test;
13. perform a second installation target and confirm artifact reuse.

The smoke record must include:

```text
Graphene commit
OS/architecture
Minecraft version ID
Java major/vendor
install result
launch result
known limitations
```

Do not use "latest" as the acceptance identifier.

---

## 62. Dependency Additions Policy

Phase 1 may require narrowly scoped workspace dependencies.

Likely categories:

```text
regex engine
ZIP/JAR reader
URL parser/validator
Tokio process support
```

Requirements:

- one dependency per clear responsibility;
- no UI dependencies;
- no second HTTP stack;
- no shell-command helper that encourages string execution;
- no archive extractor used without Graphene path/size safety wrappers;
- dependency versions are workspace-pinned consistently;
- license/security review before merge.

If an async trait object is required, prefer stable-language patterns or one narrowly selected
abstraction rather than spreading boxed futures throughout public APIs.

---

## 63. Implementation Workstreams

Phase 1 implementation should proceed in this order.

### Workstream 1 — Phase 0 Gate and Workspace Activation

Implement:

- verify Phase 0 final pass;
- add Phase 1 crates;
- add dependency checks;
- root facade placeholders only when backed by real types.

Exit:

- workspace compiles with empty/minimal new crates;
- dependency graph is valid;
- no business feature implemented in the wrong crate.

### Workstream 2 — Minecraft Domain

Implement:

- IDs/types;
- normalized metadata;
- rules;
- arguments;
- Maven coordinates;
- assets;
- libraries;
- natives;
- inheritance;
- `ResolvedMinecraft`.

Exit:

- deterministic fixture-only resolution tests pass;
- crate has no network/filesystem/process dependency.

### Workstream 3 — Mojang Provider and Metadata Acquisition

Implement:

- manifest DTO;
- version DTO;
- asset-index DTO;
- provider conversion;
- bounded manifest GET;
- verified version/asset-index acquisition;
- local endpoint test configuration.

Exit:

- local fixture server resolves an explicit version into domain metadata;
- no DTO escapes the provider crate.

### Workstream 4 — Minimal Instance and Install Planning

Implement:

- create-only instance descriptor/layout;
- install receipt;
- `InstallRequest`;
- `InstallPlan`;
- complete artifact enumeration;
- plan validation;
- plan snapshot.

Exit:

- plan generation enumerates all fixture client/library/asset/native/logging requirements before
  instance mutation.

### Workstream 5 — Transactional Install Executor

Implement:

- acquisition port adapter to Phase 0 artifact service;
- bounded artifact scheduling;
- shared materialization;
- staging;
- native extraction;
- metadata writes;
- validation;
- final create-only commit;
- cancellation.

Exit:

- fixture install succeeds;
- failures/cancellation leave no committed target;
- second target reuses cache.

### Workstream 6 — Java Runtime

Implement:

- discovery;
- probe;
- normalization;
- compatibility;
- selection;
- fake-Java support.

Exit:

- deterministic cross-platform Java tests pass without system Java.

### Workstream 7 — Launch Planning

Implement:

- ephemeral `LaunchSession`;
- placeholders;
- rule context;
- classpath;
- JVM/game argv;
- redacted plan representation.

Exit:

- fixture installed instance produces a deterministic redacted `LaunchPlan`;
- plan generation is offline.

### Workstream 8 — Process Runtime

Implement:

- platform process primitives;
- spawn;
- output drain;
- bounded lifecycle stream;
- wait;
- kill;
- exit result.

Exit:

- fake Java receives the expected argv and emits observable output/exit events;
- no shell is involved.

### Workstream 9 — End-to-End Hardening

Implement:

- complete fixture E2E;
- cancellation race tests;
- archive security tests;
- architecture checks;
- docs;
- manual real Vanilla smoke record.

Exit:

- every Phase 1 checklist item is satisfied.

---

## 64. Public API Stability Strategy

Phase 1 should avoid prematurely freezing every low-level type.

### Internal Module Ownership

Phase 1 crate roots are public facades, not business-logic containers. The implementation keeps
Minecraft identity/metadata/rules/resolution, instance schema concepts, Java
discovery/probing/selection, Mojang provider DTO/normalization/policy, install
planning/execution/archive handling, and launch planning/process lifecycle in cohesive private
modules. `lib.rs` files deliberately re-export the documented crate API without making those
internal modules public. Unit tests are colocated with the module that owns the behavior they
exercise.

Stability priority:

### High

Should be designed as durable contracts:

```text
ResolvedMinecraft
InstallRequest
InstallPlan
InstalledInstance
JavaRuntime
JavaRequirement
LaunchSession
LaunchRequest
LaunchPlan
RunningGame
```

### Medium

May remain crate-level or non-exhaustive:

```text
provider configuration
metadata normalization helpers
internal plan actions
archive extraction details
process event implementation details
```

### Private

Must remain private:

```text
Mojang DTOs
reqwest responses
ZIP library entry types
Tokio child internals
filesystem lock implementation
```

Public enums likely to grow should remain `#[non_exhaustive]` where appropriate.

---

## 65. Compatibility with Later Phases

### 65.1 Phase 2 — Authentication

Phase 2 must be able to produce `LaunchSession` from an account without changing Minecraft launch
argument logic.

### 65.2 Phase 3 — Loaders

Loaders must modify/compose Minecraft resolution so the final result is still
`ResolvedMinecraft`.

`graphene-launch` must not gain:

```text
if fabric ...
if forge ...
if neoforge ...
```

### 65.3 Phase 4 — Instance Engine

Phase 4 expands create-only Phase 1 instances with:

- mutation locks;
- configuration;
- clone/delete/rename;
- lockfile;
- verification;
- repair.

Phase 1 install receipt must therefore be schema-versioned and migration-friendly.

### 65.4 Phase 5+ — Content and Packs

Content/modpack installation must reuse:

- Artifact;
- InstallPlan concepts;
- transaction primitives;
- instance boundaries.

Phase 1 must not make Vanilla a special one-off installer that cannot be extended.

---

## 66. Documentation Deliverables During Implementation

When Phase 1 code is implemented, the following documentation should accompany it:

```text
docs/PHASE_1_IMPLEMENTATION_PLAN.md   # this normative implementation plan
docs/PHASE_1_API.md                   # actual implemented API behavior
docs/PHASE_1_SECURITY_REVIEW.md       # completed review/checklist
```

Add an ADR if implementation changes a major architecture decision, especially:

- acquisition dependency inversion;
- install transaction publication semantics;
- process ownership/lifecycle;
- persistence schema that later phases must inherit.

The implementation plan must not be silently rewritten to match accidental code behavior.

---

## 67. Review Checklist

Every Phase 1 pull request should ask:

### Architecture

- Does the owning crate make sense?
- Did a domain crate gain a concrete provider/network/UI dependency?
- Did service become a logic dump?
- Is there a new cycle?
- Does the code keep Mojang DTOs at the adapter boundary?

### Resolution

- Is behavior deterministic?
- Are rules/inheritance tested?
- Are paths derived safely?
- Are unsupported metadata cases explicit?

### Installation

- Is the full plan known before final instance mutation?
- Does every remote immutable artifact use the Phase 0 acquisition pipeline?
- Are shared files immutable and verified?
- Is staging separate from committed state?
- Does cancellation preserve committed-state validity?

### Java

- Is process probing direct argv?
- Is timeout/output bounded?
- Is selection deterministic?

### Launch

- Is `LaunchPlan` inspectable before execution?
- Are secrets classified/redacted?
- Is classpath deterministic?
- Is a shell completely absent?

### Testing

- Does CI remain offline/deterministic?
- Are failure/cancellation cases tested?
- Are platform-specific differences explicit?

---

## 68. Phase 1 Exit Checklist

Phase 1 may be marked complete only when every item is checked.

### Architecture

- [ ] Phase 0 final gate is green.
- [x] New crate dependency graph is acyclic.
- [x] Architecture checker covers all Phase 1 crates.
- [x] No UI dependency exists in backend crates.
- [x] Mojang DTOs do not escape `graphene-providers`.
- [x] Install/launch do not depend on `graphene-service`.

### Minecraft

- [ ] Official-compatible version manifest is normalized.
- [ ] Explicit version selection works.
- [ ] Version JSON is normalized.
- [ ] Inheritance is deterministic and cycle-safe.
- [ ] Rules are deterministic and fixture-tested.
- [ ] Modern arguments are supported.
- [ ] Legacy argument parsing required by supported fixtures is supported.
- [ ] Maven coordinates are validated.
- [ ] Libraries are resolved.
- [ ] Natives are selected.
- [ ] Asset index and objects are resolved.
- [ ] Logging configuration is resolved.
- [ ] Java requirement is normalized.
- [ ] `ResolvedMinecraft` is snapshot-tested.

### Installation

- [ ] `InstallRequest` is validated.
- [ ] `InstallPlan` is complete before committed instance mutation.
- [ ] Plan validation rejects unsafe/conflicting targets.
- [ ] All remote immutable artifacts use the shared Phase 0 acquisition path.
- [ ] Artifact scheduling is bounded.
- [ ] Shared immutable materialization is safe.
- [ ] Instance staging is isolated.
- [ ] Native extraction is traversal/size/symlink safe.
- [ ] `instance.json` is schema-versioned.
- [ ] `install.json` is schema-versioned and provider-neutral.
- [ ] Staged validation runs before commit.
- [ ] Final commit is create-only and cancellation-safe.
- [ ] Cancelled/failed install leaves no committed instance.
- [ ] Repeated installation reuses valid cache/shared artifacts.

### Java

- [ ] Local discovery works on supported OS abstractions.
- [ ] Candidate probing is direct process execution.
- [ ] Probe timeout/output are bounded.
- [ ] Java major parsing is tested.
- [ ] Candidate deduplication is deterministic.
- [ ] Compatible selection is deterministic.
- [ ] No compatible runtime returns a structured error.
- [ ] Managed Java is not accidentally implemented in Phase 1.

### Launch

- [ ] Launch session is ephemeral and not persisted.
- [ ] Secret values are redacted.
- [ ] Launch planning requires no metadata network access.
- [ ] Placeholder substitution is complete for Tier A fixture.
- [ ] Classpath is deterministic.
- [ ] JVM args are deterministic.
- [ ] Game args are deterministic.
- [ ] `LaunchPlan` is redacted/snapshot-testable.
- [ ] Process spawn uses argv and no shell.
- [ ] stdout/stderr are drained concurrently.
- [ ] Process output queue is bounded.
- [ ] PID/start/exit are observable.
- [ ] wait/kill behavior is tested.

### Security

- [ ] Remote JSON body limits are enforced.
- [ ] Provider URL/scheme policy is enforced.
- [ ] All provider-controlled paths are contained.
- [ ] Hash/size verification is preserved.
- [ ] Native archive attacks are tested.
- [ ] Secrets never enter logs/errors/metadata.
- [ ] Blocking filesystem/archive work does not stall async I/O workers.

### Tests and CI

- [ ] Unit test matrix is implemented.
- [ ] Full local HTTP fixture install passes.
- [ ] Cache-reuse integration test passes.
- [ ] Cancellation matrix passes.
- [ ] Failure matrix passes.
- [ ] Fake Java tests pass.
- [ ] Redacted plan snapshots pass.
- [ ] Windows CI passes.
- [ ] Linux CI passes.
- [ ] macOS CI passes.
- [ ] `cargo fmt` passes.
- [ ] `cargo check` passes.
- [ ] `cargo test` passes.
- [ ] `cargo clippy -D warnings` passes.
- [ ] `cargo doc` passes.
- [x] architecture checker passes.
- [ ] documented real Vanilla smoke procedure passes.

---

## 69. Definition of Done

Phase 1 is done when this statement is demonstrably true:

> Graphene can turn an explicit official Vanilla Minecraft version into a provider-neutral resolved
> model, turn that model into a deterministic installation plan, execute the plan through the
> verified shared artifact pipeline into a transactionally committed isolated instance, select a
> compatible local Java runtime, reconstruct launch state from committed local metadata without
> network access, generate a redacted deterministic `LaunchPlan`, and execute that plan as a direct
> Java process with observable output and lifecycle, while preserving cancellation, integrity,
> filesystem, dependency, and UI-independence invariants established in Phase 0.

The phase is **not** done merely because one ad-hoc command line starts Minecraft.

It is done when the complete engine path is reusable, deterministic, testable, bounded, and ready
for Phase 2 authentication and Phase 3 loader composition without redesigning the core install or
launch pipeline.

---

## 70. Phase 1 Architectural North Star

The intended Phase 1 architecture is:

```text
                         Graphene Facade
                               |
                         Service Wiring
                               |
          +--------------------+--------------------+
          |                    |                    |
          v                    v                    v
      Minecraft             Install              Launch
          |                    |                    |
          |                    |               +----+----+
          |                    |               |         |
          v                    v               v         v
    Mojang Provider       Instance          Java      Platform
          |                    |
          v                    v
       Network             Storage
          |                    |
          +---------+----------+
                    |
                   Core
```

The provider flow is:

```text
Mojang JSON
    |
    v
provider DTO
    |
    v
normalization
    |
    v
MinecraftVersionMetadata
    |
    v
inheritance + rules + assets/libraries/natives
    |
    v
ResolvedMinecraft
```

The installation flow is:

```text
InstallRequest
      |
      v
Resolve metadata + asset index
      |
      v
ResolvedMinecraft
      |
      v
InstallPlan
      |
      v
Phase 0 Artifact Acquisition
      |
      v
Shared immutable materialization
      |
      v
Instance staging
      |
      v
Native extraction + metadata
      |
      v
Validation
      |
      v
Create-only commit
```

The launch flow is:

```text
Installed instance receipt
        +
External LaunchSession
        +
Local Java candidates
        |
        v
JavaSelection
        |
        v
Rule/placeholder resolution
        |
        v
LaunchPlan
        |
        v
Direct process spawn
        |
        v
RunningGame
        |
        +--> stdout/stderr
        +--> PID
        +--> exit
```

If Phase 1 preserves these three flows and their boundaries, Graphene will have the correct base for
accounts, loaders, mutable instances, content, modpacks, diagnostics, and multiple UI hosts.
