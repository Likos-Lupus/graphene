# Graphene Phase 0 — Detailed Foundation Implementation Plan

**Status:** Approved implementation plan  
**Phase:** 0 — Foundation  
**Project version baseline:** 0.1.x  
**Primary implementation language:** Rust 2024  
**Implementation state:** Documentation only; no Phase 0 source changes are included in this
revision  
**Normative parent documents:** `PROJECT_SPECIFICATION.md`, `ARCHITECTURE.md`,
`SCOPE_AND_BOUNDARIES.md`, `ROADMAP.md`

---

## 1. Purpose

Phase 0 establishes the technical foundation on which every later Graphene capability depends.

It is deliberately not a Minecraft feature phase. It exists to make the Phase 1 Vanilla
install-to-launch vertical slice possible without introducing architectural shortcuts that later
need to be removed.

Phase 0 must establish:

- the Cargo workspace and stable root facade;
- foundational domain primitives;
- structured errors and diagnostics;
- one operation/progress/event/cancellation model;
- platform and filesystem primitives;
- the initial Graphene data-root layout;
- reusable HTTP and artifact download infrastructure;
- artifact size/hash verification;
- safe temporary-file and cache commit semantics;
- minimal service composition;
- deterministic foundation tests;
- cross-platform CI and architecture checks.

The output is a **verified UI-independent launcher foundation**, not a user-facing launcher feature.

---

## 2. Phase 0 Objective

The foundation must prove this flow:

```text
GrapheneBuilder
      |
      v
Validate configuration
      |
      v
Initialize data root
      |
      v
Construct service context
      |
      +-------------------------+
      |                         |
      v                         v
Operation runtime          Network client
      |                         |
      v                         v
Events / progress         Artifact request
      |                         |
      v                         v
Cancellation             Stream to temp file
                                |
                                v
                         Size/hash verification
                                |
                                v
                           Safe cache commit
```

This must work without Minecraft metadata, Java discovery, authentication, loaders, instance
installation, launch planning, Tauri, Slint, or another UI framework.

---

## 3. Phase 0 Success Statement

Phase 0 is successful when:

> A host can construct a Graphene engine using an explicit data root, start a cancellable
> long-running operation, observe structured nested progress and state events, download a
> deterministic test artifact through the shared network layer, verify its size/hash, safely commit
> it
> into the Graphene cache, and receive stable structured errors on failure, with no UI dependency
> and
> no valid-looking partial output after cancellation.

This is the primary acceptance statement for the phase.

---

## 4. Scope

### 4.1 In Scope

1. Cargo workspace conversion.
2. Root `graphene` facade preservation.
3. Foundation crate activation.
4. Strong IDs and common value objects.
5. Artifact primitives.
6. Hash/integrity primitives.
7. Structured error contract.
8. Generic diagnostic contract.
9. Operation lifecycle state.
10. Nested progress.
11. Cooperative cancellation.
12. Typed event publication/subscription.
13. OS and architecture normalization.
14. Filesystem/path helpers.
15. Graphene data-root initialization.
16. Initial storage layout and layout version marker.
17. HTTP client configuration.
18. TLS/redirect/timeout policy.
19. Proxy abstraction.
20. Retry/backoff policy.
21. Streamed downloads.
22. Temporary download handling.
23. Size/hash verification.
24. Deterministic cache paths.
25. Safe cache commit.
26. Bounded download concurrency.
27. Structured tracing foundation.
28. Foundation configuration validation.
29. Unit/integration/property/failure tests.
30. CI and architecture checks.
31. Rustdoc and implementation-facing documentation.

### 4.2 Explicitly Out of Scope

Phase 0 must not implement:

- Mojang version manifests or Minecraft version JSON;
- Maven libraries/assets/natives;
- `ResolvedMinecraft`;
- `InstallPlan` or installation transactions;
- instance create/delete/clone;
- Java discovery or managed Java;
- Microsoft or offline account workflows;
- Fabric/Forge/NeoForge/Quilt;
- Modrinth or CurseForge;
- modpack import/export;
- `LaunchPlan`;
- Minecraft process execution;
- crash analysis;
- UI commands/controllers;
- launcher self-update;
- dynamic plugins.

Types may be prepared for future extension, but Phase 0 must not contain fake placeholder business
logic for later phases.

---

## 5. Repository Transition

Current baseline:

```text
graphene/
├── Cargo.toml
├── Cargo.lock
└── src/
    └── lib.rs
```

Target Phase 0 repository:

```text
graphene/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── CONTRIBUTING.md
├── LICENSE
├── src/
│   └── lib.rs
├── crates/
│   ├── graphene-core/
│   ├── graphene-platform/
│   ├── graphene-network/
│   ├── graphene-storage/
│   └── graphene-service/
├── tests/
│   ├── fixtures/
│   │   └── network/
│   └── integration/
└── docs/
```

### 5.1 Phase 0 Crates

The detailed plan activates five internal crates.

#### `graphene-core`

Owns stable foundation domain types:

- IDs;
- artifact primitives;
- hash/integrity values;
- errors;
- diagnostics;
- operations;
- progress;
- cancellation;
- event payloads.

It must remain independent from HTTP, filesystem implementations, providers, and UI frameworks.

#### `graphene-platform`

Owns OS-facing capabilities:

- normalized OS/architecture;
- path/filesystem helpers;
- directory creation;
- safe replacement primitives;
- platform capability detection.

It must not own launcher domain policy.

#### `graphene-network`

Owns generic transport and download behavior:

- HTTP configuration;
- timeouts;
- redirects;
- proxy policy;
- retries;
- streaming;
- verification handoff;
- source fallback;
- download concurrency.

It must not know Mojang, Modrinth, loaders, or instances.

#### `graphene-storage`

Phase 0 activates the foundation portion of the storage context because the roadmap requires the
data root and initial layout.

It owns:

- root initialization;
- layout marker;
- cache paths;
- temporary paths;
- safe commit helpers.

It does **not** yet own instance/account repositories, SQLite, or mature migrations.

#### `graphene-service`

Introduced as a deliberately small composition layer so the root facade does not become a service
locator or implementation dump.

It owns:

- dependency construction;
- shared service context;
- operation runtime wiring;
- network/storage/platform composition.

No Minecraft business logic belongs here.

---

## 6. Workspace Dependency Direction

Required dependency direction:

```text
                    graphene
                       |
                       v
                graphene-service
                 /      |      \
                v       v       v
     graphene-network  graphene-storage
               \         /
                \       /
                 v     v
             graphene-platform
                    |
                    v
              graphene-core
```

Practical policy:

```text
graphene-core
    -> std + narrowly selected foundational libraries only

graphene-platform
    -> graphene-core

graphene-network
    -> graphene-core
    -> graphene-platform only where actual platform integration is required

graphene-storage
    -> graphene-core
    -> graphene-platform

graphene-service
    -> graphene-core
    -> graphene-platform
    -> graphene-network
    -> graphene-storage

graphene
    -> graphene-core
    -> graphene-service
```

Forbidden edges include:

```text
graphene-core -> graphene-network
graphene-core -> graphene-storage
graphene-platform -> graphene-network
graphene-network -> graphene-service
graphene-storage -> graphene-service
any Phase 0 crate -> tauri
any Phase 0 crate -> slint
```

The graph must remain acyclic.

---

## 7. Proposed Module Layout

### 7.1 `graphene-core`

```text
src/
├── lib.rs
├── id.rs
├── artifact.rs
├── hash.rs
├── error.rs
├── diagnostic.rs
└── operation/
    ├── mod.rs
    ├── event.rs
    ├── progress.rs
    ├── state.rs
    └── cancellation.rs
```

### 7.2 `graphene-platform`

```text
src/
├── lib.rs
├── os.rs
├── arch.rs
├── filesystem.rs
└── paths.rs
```

### 7.3 `graphene-network`

```text
src/
├── lib.rs
├── client.rs
├── config.rs
├── retry.rs
├── proxy.rs
├── source.rs
├── cache.rs
└── download/
    ├── mod.rs
    ├── request.rs
    ├── result.rs
    ├── manager.rs
    ├── stream.rs
    └── verify.rs
```

### 7.4 `graphene-storage`

```text
src/
├── lib.rs
├── root.rs
├── layout.rs
├── atomic.rs
├── cache.rs
└── temp.rs
```

### 7.5 `graphene-service`

```text
src/
├── lib.rs
├── builder.rs
├── context.rs
└── services.rs
```

---

## 8. Strong Identifier Model

Phase 0 establishes the typed identifier pattern used throughout Graphene.

Required initial IDs:

```text
OperationId
ArtifactId
```

Future IDs should follow the same pattern:

```text
InstanceId
AccountId
ProviderId
ComponentUid
```

Requirements:

- cheap to clone;
- equality/hash comparable;
- parseable and displayable;
- serializable where persistence requires it;
- opaque to callers;
- strongly typed instead of passing raw `String`/UUID values everywhere.

The public API should expose typed wrappers even if the internal representation is UUID-like.

---

## 9. Artifact Foundation

`Artifact` is introduced in Phase 0 because every later acquisition path depends on it.

Conceptual direction:

```rust
pub struct Artifact {
    pub id: ArtifactId,
    pub kind: ArtifactKind,
    pub sources: Vec<ArtifactSource>,
    pub integrity: ArtifactIntegrity,
    pub expected_size: Option<u64>,
    pub cache_policy: CachePolicy,
}
```

Phase 0 deliberately avoids embedding instance materialization paths in `Artifact`. Installation
owns where a verified artifact is materialized later.

### 9.1 Artifact Source

A source models:

- URL;
- deterministic priority/order;
- optional source/mirror identity;
- only generic request metadata that is safe for the network boundary.

Sensitive authorization data must not be part of normal debug output.

### 9.2 Integrity

At minimum the foundation must represent:

- SHA-1;
- SHA-256.

Hashing is incremental and does not load whole artifacts into memory.

Normalized textual form is lowercase hexadecimal.

Parsing rejects:

- wrong digest length;
- non-hex characters;
- algorithm/digest mismatch.

### 9.3 Verification Rule

When trustworthy expected integrity data exists, the artifact is not valid until it is verified.

The downloader must never commit an unverified file under a path that consumers interpret as a valid
cache object.

## 10. Error Model

Phase 0 defines the error contract that all later crates extend.

The public error must distinguish:

1. stable machine-readable code;
2. broad category;
3. structured context;
4. source error chain for debugging;
5. human-readable developer message.

Conceptual model:

```rust
pub struct GrapheneError {
    pub code: ErrorCode,
    pub kind: ErrorKind,
    pub context: ErrorContext,
    pub source: Option<BoxError>,
}
```

Initial categories:

```text
Configuration
Platform
Filesystem
Network
Timeout
Cancelled
Integrity
Storage
Internal
```

Initial stable codes should include at least:

```text
CONFIG_INVALID
DATA_ROOT_INVALID
DIRECTORY_CREATE_FAILED
FILE_OPEN_FAILED
FILE_WRITE_FAILED
FILE_RENAME_FAILED
NETWORK_REQUEST_FAILED
NETWORK_TIMEOUT
NETWORK_STATUS_ERROR
NETWORK_REDIRECT_REJECTED
DOWNLOAD_CANCELLED
DOWNLOAD_SIZE_MISMATCH
HASH_MISMATCH
CACHE_COMMIT_FAILED
OPERATION_CANCELLED
INTERNAL_INVARIANT_VIOLATION
```

Rules:

- hosts must not parse human-readable strings;
- cancellation is distinct from generic failure;
- retryability should be structural where useful;
- secret data never appears in error context;
- source errors may be retained without leaking concrete transport types through the main API;
- integrity failure can never be silently converted to success.

---

## 11. Diagnostic Model

Errors and diagnostics are intentionally separate.

An error means an operation failed to complete as requested.

A diagnostic is structured evidence a host may present independently of localization.

Conceptual form:

```rust
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: DiagnosticSeverity,
    pub parameters: DiagnosticParameters,
}
```

Initial severity levels:

```text
Info
Warning
Error
Critical
```

Phase 0 establishes the shape, not a large diagnostic catalog. Java, Minecraft, content, and crash
diagnostics are later phases.

---

## 12. Operation Lifecycle

All long-running Graphene work must share one lifecycle.

Required states:

```text
Created
Queued
Running
Cancelling
Succeeded
Failed
Cancelled
```

Allowed high-level transitions:

```text
Created -> Queued
Created -> Running
Queued  -> Running

Running -> Succeeded
Running -> Failed
Running -> Cancelling

Cancelling -> Cancelled
Cancelling -> Failed

Created/Queued -> Cancelled
```

Terminal states:

```text
Succeeded
Failed
Cancelled
```

A terminal operation never returns to a non-terminal state.

A host-facing operation handle should conceptually support:

```text
operation_id()
cancel()
events()/subscribe()
current_state()
await_result()
```

The exact Rust surface may evolve, but these capabilities must be available.

---

## 13. Cancellation

Cancellation is a cross-cutting foundation contract and must exist before download/install code.

Required properties:

- cooperative;
- idempotent;
- cheap to clone/share;
- propagated into child work;
- observable at meaningful checkpoints;
- distinguishable from failure;
- safe during I/O and verification.

Minimum cancellation checkpoints:

```text
before request
after headers
between body chunks
before expensive verification
during large verification where practical
before cache commit
before starting child tasks
```

### 13.1 Cancellation Cleanup

A cancelled download must not leave a partial file under the final cache object path.

Temporary state may remain only when deliberately modeled as resumable data. Phase 0 should favor
correctness and clear cleanup over sophisticated cross-session resume behavior.

---

## 14. Progress Model

The model must support future installation trees without introducing another progress system.

Minimum forms:

```rust
pub enum Progress {
    Indeterminate,
    Items { completed: u64, total: Option<u64> },
    Bytes { completed: u64, total: Option<u64> },
}
```

It must also support:

- stage changes;
- parent/child operations;
- nested work;
- unknown totals.

Invariants:

- `completed <= total` when total is known;
- progress is monotonic within one stage;
- UI-neutral structured information is preferred over localized status strings.

Example future shape already supported by the model:

```text
Install Instance
├── Resolve metadata       done
├── Download               62%
│   ├── client             done
│   ├── libraries          73%
│   └── assets             58%
└── Commit                 waiting
```

---

## 15. Event Model

Graphene publishes typed events; it does not invoke UI-framework callbacks.

Phase 0 event classes should cover:

```text
OperationCreated
OperationStateChanged
OperationStageChanged
OperationProgress
OperationCompleted
OperationFailed
OperationCancelled
```

Requirements:

- every event carries `OperationId`;
- child operations may carry `parent_id`;
- terminal event is emitted at most once;
- per-operation ordering is coherent;
- slow observers must not indefinitely block download I/O;
- event capacity/backpressure policy is documented.

Recommended backpressure semantics:

- state transitions and terminal state are retained/reliable within the operation model;
- high-frequency progress events may be coalesced;
- queues are bounded;
- callers can query current state if they subscribe late.

---

## 16. Async Runtime Policy

Graphene is asynchronous for networking and future installation workflows.

Rules:

- public service operations should be async where appropriate;
- do not create an uncontrolled global runtime;
- runtime-specific handles remain internal when possible;
- one process can construct multiple independent Graphene engines;
- blocking file hashing or expensive filesystem work must not stall async I/O threads.

If Tokio is selected, Tokio-specific domain leakage should be minimized.

---

## 17. Platform Foundation

Phase 0 normalizes host platform information.

At minimum:

```text
OperatingSystem:
  Windows
  Linux
  MacOS
  Unknown/Other only if required for forward compatibility

Architecture:
  X86_64
  AArch64
  other values only where there is a real support requirement
```

The platform crate also provides:

- managed-path helpers;
- safe directory creation;
- canonical/normalized root handling where appropriate;
- safe replace/rename capability;
- platform error conversion.

Deferred:

- full process execution;
- Java discovery;
- OS keyring;
- trash/recycle bin;
- process lifecycle for Minecraft.

---

## 18. Data Root and Storage Layout

Phase 0 reserves the top-level Graphene layout:

```text
graphene-data/
├── config/
├── instances/
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
└── logs/
```

Not every directory has a Phase 0 consumer, but ownership and meaning are established now.

### 18.1 Root Selection

`GrapheneBuilder` must accept an explicit root path.

A platform-default location may exist as convenience, but tests and reusable library behavior must
not depend on a global user directory.

Explicit root always wins.

### 18.2 Initialization

Initialization should:

1. validate/normalize the requested root;
2. create required directories;
3. verify basic write capability where practical;
4. create/read a layout version marker;
5. preserve unknown user files;
6. be idempotent.

Recommended marker:

```text
graphene-data/.graphene-layout.json
```

Conceptually:

```json
{
  "layout_version": 1
}
```

This marker is only for top-level storage layout evolution. It is not an instance lockfile and not a
database migration schema.

---

## 19. Atomic Write and Commit Semantics

Foundation write primitives must establish the safety model reused by installation later.

Preferred sequence:

```text
write temporary file
      |
      v
flush
      |
      v
verify/validate
      |
      v
rename or safest platform replacement
      |
      v
committed file
```

Required guarantees:

- the previous committed object is not truncated before replacement is ready;
- a failed write never produces a valid-looking partial final object;
- temporary filenames are recognizable;
- abandoned temporary state can be cleaned safely;
- platform differences are documented.

Full crash-durability policy (`fsync`, directory sync) may be refined later, but atomicity and
validity semantics must be clear in Phase 0.

---

## 20. Filesystem Safety

Managed-path APIs must carefully handle:

- `..` traversal;
- unexpected absolute paths where relative paths are required;
- platform-invalid path components;
- cross-filesystem rename limitations;
- symbolic-link surprises where security-sensitive;
- accidental deletion outside the Graphene root.

Archive extraction is a later phase, but Phase 0 path helpers should be reusable by secure
extractors.

---

## 21. Network Configuration

The network layer exposes Graphene-owned policy types rather than raw HTTP-client configuration as
the primary API.

Conceptual configuration:

```rust
pub struct NetworkConfig {
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub retry_policy: RetryPolicy,
    pub proxy: ProxyPolicy,
    pub redirect_policy: RedirectPolicy,
    pub user_agent: String,
    pub max_concurrent_downloads: usize,
}
```

Configuration rules:

- conservative defaults;
- invalid limits rejected during build;
- bounded timeouts;
- bounded redirects;
- no public option that casually disables TLS verification;
- raw concrete HTTP client types stay internal.

---

## 22. TLS and Redirect Policy

Production networking must:

- use HTTPS/TLS with certificate validation;
- have a bounded redirect count;
- reject unsupported or dangerous redirect behavior according to explicit policy;
- keep one coherent TLS implementation where practical.

Development tests may use a local HTTP fixture server.

Disabling certificate validation is not an ordinary end-user feature.

---

## 23. Proxy Model

The configuration surface should reserve:

```text
System/default proxy behavior
No proxy
Explicit HTTP/HTTPS proxy
```

Proxy credentials are sensitive:

- never included in tracing fields;
- never included in error context;
- never emitted by `Debug` output of configuration types.

SOCKS may be added if the selected transport supports it cleanly, but it is not mandatory for the
minimum Phase 0 exit.

---

## 24. Retry Policy

Retries must be explicit, bounded, and classification-based.

Likely retry candidates:

- transient connection reset;
- temporary DNS/network failure;
- selected 5xx responses;
- HTTP 429 when bounded retry timing can be honored.

Do not blindly retry:

- normal permanent 4xx failures;
- invalid configuration;
- explicit cancellation;
- integrity mismatch as if the artifact were valid.

Use bounded exponential backoff with jitter or an equivalent strategy.

There is no infinite retry mode in the foundation.

## 25. Download Manager

Phase 0 implements the reusable artifact acquisition path.

Required flow:

```text
DownloadRequest
      |
      v
Resolve ordered sources
      |
      v
Check cache
      |
      +--> valid -> return cached result
      |
      v
Create temporary file
      |
      v
Stream HTTP body
      |
      +--> byte progress
      |
      +--> cancellation
      |
      v
Verify expected size
      |
      v
Verify expected hash
      |
      v
Safe cache commit
      |
      v
DownloadResult
```

The request contains only generic acquisition information:

- artifact identity;
- ordered candidate sources;
- expected size;
- expected hashes;
- cache policy;
- operation context.

It must not contain Minecraft-specific concepts.

### 25.1 Source Fallback

When multiple sources exist:

- source order is deterministic;
- policy decides which failures may fall through;
- per-source evidence is retained in structured context;
- invalid content never overwrites a valid cache object;
- authentication secrets are redacted from context.

### 25.2 Cache Hit

A file existing at the expected path is not automatically trusted.

The cache path must be derived deterministically, and existing data must be accepted only according
to the artifact integrity/cache policy.

### 25.3 Duplicate Requests

Concurrent requests for the same object must not corrupt the cache.

The first implementation may use:

- in-process keyed deduplication; or
- race-safe independent temporary downloads plus atomic final commit.

Physical download deduplication is preferred, but **final-state correctness is mandatory**.

---

## 26. Cache Foundation

Recommended content-addressed direction:

```text
cache/
└── objects/
    ├── sha1/
    │   └── <prefix>/<digest>
    └── sha256/
        └── <prefix>/<digest>
```

Temporary transfers live separately:

```text
cache/downloads/temporary/
```

Rules:

- committed objects are immutable;
- temporary paths never equal final paths;
- verified digest identity is preferred over mutable URL identity;
- a correct existing destination resolves a commit race as success;
- cache deletion may reduce performance but cannot delete authoritative instance state.

The exact shard depth is an implementation detail, but it must be deterministic and tested.

---

## 27. Download Concurrency

The download manager must use bounded concurrency.

Phase 0 establishes:

- global maximum active downloads;
- operation-aware child work;
- cancellation propagation;
- bounded event pressure.

No plan should be able to create unbounded tasks simply because it contains many artifacts.

Adaptive bandwidth scheduling and advanced per-host throttling are deferred until real workloads
justify them.

---

## 28. Resource Limits

Foundation code should explicitly bound:

- redirects;
- retry attempts;
- event queue size;
- download concurrency;
- in-memory response buffering;
- temporary file behavior.

Artifact bodies stream directly to disk.

Metadata endpoints added later may buffer bounded JSON responses, but the network foundation must
not encourage whole-artifact buffering.

---

## 29. Configuration Foundation

Phase 0 configuration includes only foundation concerns:

```text
data_root
network timeouts
retry policy
proxy policy
redirect policy
download concurrency
event channel capacity/policy
optional tracing integration
```

Rules:

- validate configuration before engine construction completes;
- defaults are explicit and documented;
- invalid zero/out-of-range values return structured configuration errors;
- configuration is effectively immutable after construction unless runtime mutation has a concrete
  later requirement;
- concrete HTTP/runtime implementation types are not part of the stable facade.

---

## 30. Graphene Builder and Facade

Phase 0 establishes the construction pattern that later phases extend.

Conceptual direction:

```rust
let graphene = Graphene::builder(data_root)
.network(network_config)
.build()
.await?;
```

Builder responsibilities:

1. validate configuration;
2. determine platform information;
3. initialize storage root;
4. construct network client;
5. construct operation/event runtime;
6. construct foundation service context;
7. return a fully initialized `Graphene`.

A partially initialized engine must not escape on failure.

The root crate:

- re-exports selected stable types;
- exposes `Graphene`/`GrapheneBuilder`;
- hides concrete transport/channel/runtime internals;
- contains minimal logic.

---

## 31. Thread Safety and Ownership

The foundation should allow multiple concurrent host tasks and multiple independent engine
instances.

Where appropriate, service handles should be:

- `Send`;
- `Sync`;
- cheap to clone through shared ownership.

The project must avoid:

- global mutable singleton state;
- global data-root assumptions;
- hidden process-wide configuration;
- static caches that make tests interfere with each other.

This must be a valid use case:

```text
process
├── Graphene(root A)
└── Graphene(root B)
```

with no state collision.

---

## 32. Logging and Tracing

Phase 0 establishes structured tracing through a standard facade.

Useful correlation fields:

```text
operation_id
artifact_id
module/context
attempt
source host
elapsed
```

Never intentionally log:

- authorization headers;
- access tokens;
- refresh tokens;
- proxy credentials;
- sensitive URL query parameters.

Host applications own final formatting, destinations, and UI presentation.

Graphene should not install a mandatory global subscriber as a hidden side effect.

---

## 33. Dependency Selection Policy

Phase 0 chooses foundational third-party crates, so selection should be conservative.

Evaluate each candidate for:

- active maintenance;
- Rust 2024 compatibility;
- ecosystem adoption;
- MSRV implications;
- Windows/Linux/macOS support;
- transitive dependency weight;
- security history;
- feature flag discipline;
- whether concrete types can remain behind Graphene boundaries.

Expected categories may include:

- async runtime;
- HTTP/TLS client;
- serialization;
- typed identity/UUID;
- hashing;
- tracing;
- temporary files;
- platform directories;
- deterministic local HTTP test server.

This document does not lock exact libraries before implementation review.

Material dependency choices that constrain architecture should receive an ADR.

### 33.1 Workspace Dependency Discipline

Prefer workspace-level dependency declarations where practical.

Disable unnecessary default features when doing so meaningfully avoids:

- duplicate TLS stacks;
- unused protocol codecs;
- unnecessary native dependencies;
- platform-specific baggage.

---

## 34. Testing Strategy

Tests are part of Phase 0 Definition of Done.

### 34.1 Unit Tests

`graphene-core`:

- typed ID roundtrip;
- hash parser/formatter;
- hash length rejection;
- progress invariants;
- operation transitions;
- terminal state protection;
- cancellation idempotency;
- structured error fields.

`graphene-platform`:

- OS/architecture normalization;
- managed relative paths;
- safe directory helpers;
- replacement behavior where testable.

`graphene-storage`:

- empty root initialization;
- repeated idempotent initialization;
- layout version reading;
- invalid layout version behavior;
- deterministic cache paths;
- temporary/final path separation.

`graphene-network`:

- retry classification;
- retry bounds;
- HTTP status mapping;
- source ordering;
- size verification;
- SHA-1/SHA-256 verification.

---

## 35. Deterministic Network Integration Tests

All Phase 0 network integration tests use a local fixture server. No test should depend on Mojang or
the public internet.

### 35.1 Successful Download

Given known fixture bytes and known digest:

```text
local server -> stream -> temp file -> verify -> cache commit
```

Assert:

- success result;
- byte progress reaches expected size;
- final bytes match fixture;
- expected digest matches;
- temporary file is removed;
- final object exists;
- repeat request can reuse valid cache.

### 35.2 Hash Mismatch

Assert:

- `HASH_MISMATCH`;
- invalid artifact is not committed;
- operation does not emit success;
- temporary state is cleaned or explicitly quarantined.

### 35.3 Size Mismatch

Assert:

- stable size-mismatch error;
- final object does not exist as valid data.

### 35.4 Cancellation

Exercise:

- cancellation before request;
- cancellation during streaming;
- cancellation immediately before commit.

Assert:

- terminal state is `Cancelled`;
- partial data is not committed;
- no later success terminal event occurs.

### 35.5 Retry

Simulate:

- transient failure then success;
- permanent non-retryable response.

Assert:

- retry count is bounded;
- transient case can succeed;
- non-retryable case does not waste retries;
- cancellation stops further retry attempts.

### 35.6 Source Fallback

First source fails, second source serves valid bytes.

Assert:

- final result succeeds;
- source sequence is deterministic;
- error context preserves useful attempt evidence;
- final return type remains provider-neutral.

### 35.7 Concurrent Duplicate Request

Start two requests targeting the same content identity.

Assert:

- final object is valid;
- no partial/corrupt destination exists;
- both callers obtain a valid result;
- implementation performs deduplication if the selected design supports it.

---

## 36. Property and Failure-Injection Tests

Property tests are recommended for:

- hash parser;
- relative managed-path sanitizer;
- progress arithmetic;
- cache-key/path determinism;
- retry bound calculations.

Failure injection should cover where practical:

- permission denied;
- interrupted connection;
- write failure;
- malformed content length;
- cancellation races;
- pre-existing valid cache destination;
- pre-existing invalid cache destination;
- invalid layout marker;
- rename/replace failure.

Foundation behavior must be designed around failure, not only the happy path.

---

## 37. Platform Matrix

Minimum CI intent:

| Platform | Architecture                      | Phase 0 expectation            |
|----------|-----------------------------------|--------------------------------|
| Windows  | x86_64                            | Build + unit/integration tests |
| Linux    | x86_64                            | Build + unit/integration tests |
| macOS    | arm64 and/or x86_64 as CI permits | Build + unit/integration tests |

Additional targets may begin as compile-only checks.

Platform-specific failures must not be hidden behind broad `cfg` exclusions without explanation.

---

## 38. CI Baseline

Recommended pull-request checks:

```text
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
```

Where appropriate:

```text
cargo test --doc
```

Additional dependency/license/security checks may be added after the dependency set is chosen.

CI rules:

- no live public-service dependency;
- deterministic fixtures;
- warnings treated seriously;
- build caches are optimization only;
- supported operating systems must actually compile the workspace.

---

## 39. Architecture Enforcement

At minimum:

- Cargo dependencies must reflect the approved direction;
- `CONTRIBUTING.md` dependency rules remain normative;
- no UI-framework dependency exists in backend crates.

Recommended automated checks:

- inspect workspace metadata for forbidden crate edges;
- deny-list Tauri/Slint in backend workspace;
- optionally enforce dependency/license policy with a standard ecosystem tool.

A custom architecture framework is unnecessary if Cargo-level boundaries make violations clear.

---

## 40. Security Review

Before Phase 0 exit, review all of the following.

### Network

- TLS certificate validation enabled;
- redirect count bounded;
- proxy credentials redacted;
- file bodies streamed;
- timeouts enforced;
- retries bounded;
- cancellation interrupts work.

### Filesystem

- managed paths cannot escape the root;
- temp files cannot appear as committed objects;
- final commit is race-safe;
- partial data cleanup is defined;
- existing valid cache data is not corrupted.

### Integrity

- digest parser validates algorithm length;
- hash verification occurs before commit;
- expected size is checked;
- integrity failures are structured.

### Logging

- credentials/tokens are excluded;
- sensitive URL fields are redacted;
- debug output of configuration/request types does not leak secrets.

### Runtime

- queues are bounded;
- concurrency is bounded;
- cancellation is idempotent;
- retries terminate;
- no global mutable singleton exists.

---

## 41. Observability

Tracing should make it possible to correlate:

```text
engine construction
operation creation
download attempt
source fallback
retry
stream completion
verification
cache hit
cache commit
operation terminal state
```

Metrics infrastructure is not required in Phase 0, but the implementation should not make later
metrics difficult.

---

## 42. Performance Direction

Phase 0 has qualitative, not benchmark-based, targets:

- stream downloads directly to disk;
- incremental hashing;
- one shared HTTP connection pool per configured engine context;
- bounded concurrency;
- deterministic cache lookup;
- no duplicate whole-file buffers;
- progress events do not overwhelm consumers;
- expensive blocking work does not block async network workers.

Formal performance benchmarks can wait until Phase 1 supplies realistic Minecraft workloads.

## 43. Documentation Requirements

Phase 0 implementation is incomplete unless its public foundation is documented.

Required:

- crate-level purpose for every activated crate;
- Rustdoc for public foundation types;
- operation state-transition invariants;
- cancellation semantics;
- event/backpressure semantics;
- cache identity and commit semantics;
- data-root layout;
- error code policy;
- security-sensitive logging rules;
- `GrapheneBuilder` construction example;
- deterministic fixture-download example.

This document remains the normative implementation plan. If implementation evidence requires a
material change, update this document and use an ADR when the architecture itself changes.

---

## 44. API Stability Policy

Graphene remains pre-1.0, so Phase 0 APIs may evolve. However, foundation contracts will be imported
by every later phase and should be designed with stable intent.

Stable-intent types include:

```text
OperationId
ArtifactId
ExpectedHash / digest representation
Artifact foundation
GrapheneError structure
ErrorCode
Diagnostic structure
OperationState
Progress
Cancellation contract
GrapheneBuilder construction direction
```

Implementation details that should remain private where possible:

```text
concrete HTTP client
connection pool type
runtime task/join handles
channel implementation
temporary-file implementation
retry/backoff implementation
```

---

## 45. Implementation Work Breakdown

Implementation should proceed dependency-first.

### P0.1 — Workspace Conversion

Deliver:

- Cargo workspace;
- preserved root facade;
- `graphene-core`;
- `graphene-platform`;
- `graphene-network`;
- `graphene-storage`;
- `graphene-service`;
- workspace dependency declarations.

Exit:

- `cargo check --workspace` succeeds;
- no functional feature work is mixed into the conversion.

### P0.2 — Core Value Types

Deliver:

- typed IDs;
- hashes/integrity;
- artifact primitives;
- structured errors;
- diagnostics;
- operation state/progress data types.

Exit:

- deterministic unit tests;
- `graphene-core` has no infrastructure dependency.

### P0.3 — Operation Runtime

Deliver:

- cancellation token abstraction;
- operation state holder/registry;
- parent-child relation;
- event publication;
- progress updates;
- terminal-state rules.

Exit:

- a synthetic long-running operation can be observed and cancelled;
- no cancellation-to-success race in tests.

### P0.4 — Platform Foundation

Deliver:

- OS/architecture normalization;
- safe path helpers;
- directory creation;
- replacement/rename primitive.

Exit:

- platform unit tests pass on supported CI targets.

### P0.5 — Storage Root

Deliver:

- explicit data-root initialization;
- top-level directory layout;
- layout version marker;
- deterministic cache/temp paths;
- idempotent initialization.

Exit:

- an empty temporary directory becomes a valid Graphene root;
- repeat initialization is safe.

### P0.6 — Network Client

Deliver:

- `NetworkConfig`;
- shared HTTP client;
- TLS policy;
- redirect policy;
- timeouts;
- proxy policy;
- retry classification/backoff.

Exit:

- local deterministic HTTP tests pass.

### P0.7 — Download / Verify / Cache

Deliver:

- streamed artifact download;
- byte progress;
- cancellation;
- source fallback;
- size checking;
- SHA-1/SHA-256 checking;
- safe cache commit;
- duplicate-request race safety.

Exit:

- fixture artifact can be downloaded, verified, cached, and reused;
- mismatch and cancellation tests prove invalid partial state cannot become committed state.

### P0.8 — Facade Construction

Deliver:

- `GrapheneBuilder`;
- minimal service context;
- root facade wiring;
- explicit isolated data root.

Exit:

- integration test constructs two independent Graphene engines using two roots.

### P0.9 — Quality Hardening

Deliver:

- CI;
- clippy/fmt/doc checks;
- cross-platform test matrix;
- architecture dependency checks;
- security review;
- documentation completion.

Exit:

- all Phase 0 acceptance criteria pass.

---

## 46. Recommended Pull Request Sequence

Keep changes reviewable:

```text
PR 1  Workspace and crate skeletons
PR 2  Core IDs, hashes, artifact, errors, diagnostics
PR 3  Operations, progress, events, cancellation
PR 4  Platform/filesystem foundation
PR 5  Storage root and cache layout
PR 6  Network client/config/retry
PR 7  Downloader, verification, cache commit
PR 8  GrapheneBuilder and service composition
PR 9  Cross-platform CI, architecture checks, final docs
```

The exact split may change, but dependency order should remain.

---

## 47. Phase 0 End-to-End Acceptance Scenario

### Given

- an empty temporary directory;
- a local deterministic HTTP fixture server;
- known artifact bytes;
- known expected size;
- a known SHA-1 and/or SHA-256;
- a `GrapheneBuilder` configured to use that temporary directory.

### When

1. Graphene is constructed;
2. the data root initializes;
3. a parent operation is created;
4. a child artifact-download operation starts;
5. the response streams to a temporary file;
6. progress events are observed;
7. size and hash are verified;
8. the verified artifact commits to cache;
9. the same artifact is requested again.

### Then

- required storage directories exist;
- typed operation/artifact IDs are used;
- parent-child relation is preserved;
- progress reaches the expected byte count;
- operation terminates in `Succeeded`;
- final bytes match the fixture;
- final hash matches;
- no temporary partial file is presented as valid;
- the repeated request can reuse the valid cache;
- all errors/results are Graphene-owned types;
- no UI framework participates.

### Cancellation Variant

Cancel during streaming.

Then:

- operation terminates in `Cancelled`;
- committed cache object is absent unless a previously valid object already existed;
- partial output remains temporary only or is cleaned;
- no `Succeeded` terminal event is emitted afterward.

---

## 48. Phase 0 Exit Checklist

### Workspace and Boundaries

- [ ] Root project is a Cargo workspace.
- [ ] Root `graphene` facade remains the consumer entry point.
- [ ] `graphene-core` exists and is infrastructure-independent.
- [ ] `graphene-platform` exists.
- [ ] `graphene-network` exists.
- [ ] `graphene-storage` foundation exists.
- [ ] `graphene-service` minimal composition exists.
- [ ] Crate dependency graph is acyclic.
- [ ] No Tauri dependency exists.
- [ ] No Slint dependency exists.
- [ ] No Minecraft business logic is implemented in Phase 0.

### Core Contracts

- [ ] Typed IDs exist.
- [ ] SHA-1/SHA-256 value types exist.
- [ ] Artifact foundation exists.
- [ ] Structured error contract exists.
- [ ] Diagnostic foundation exists.
- [ ] Operation lifecycle exists.
- [ ] Nested progress exists.
- [ ] Cooperative cancellation exists.
- [ ] Typed event stream exists.
- [ ] Terminal-state invariants are tested.

### Platform and Storage

- [ ] OS/architecture normalization exists.
- [ ] Explicit data root initializes from an empty directory.
- [ ] Initialization is idempotent.
- [ ] Layout version marker exists.
- [ ] Managed temp/final paths are distinct.
- [ ] Cache layout is deterministic.
- [ ] Safe replacement semantics are documented and tested.

### Network and Download

- [ ] Shared HTTP client exists.
- [ ] TLS certificate validation is enabled.
- [ ] Timeouts are bounded.
- [ ] Redirects are bounded.
- [ ] Retry attempts are bounded.
- [ ] Proxy policy is represented.
- [ ] Download concurrency is bounded.
- [ ] Files stream to disk.
- [ ] Byte progress is emitted.
- [ ] Cancellation interrupts downloads.
- [ ] Expected size verification works.
- [ ] SHA-1 verification works.
- [ ] SHA-256 verification works.
- [ ] Invalid downloads are not committed.
- [ ] Valid cached objects avoid unnecessary redownload.
- [ ] Racing duplicate requests cannot corrupt final cache state.

### Quality

- [ ] Foundation unit tests pass.
- [ ] Local HTTP integration tests pass.
- [ ] No test requires public internet.
- [ ] Cancellation race tests pass.
- [ ] Failure-injection coverage exists for critical boundaries.
- [ ] Formatting check passes.
- [ ] Workspace check passes.
- [ ] Workspace tests pass.
- [ ] Clippy passes under agreed warning policy.
- [ ] Rustdoc builds.
- [ ] Supported CI platforms compile and test.
- [ ] Public foundation APIs are documented.
- [ ] Security review checklist is complete.

---

## 49. Phase 0 Definition of Done

Phase 0 is done only when all of these are true at the same time:

1. The repository has the intended foundation crate boundaries.
2. `Graphene` can be constructed using an explicit isolated data root.
3. Multiple Graphene roots can coexist in one process.
4. Long-running work has one coherent lifecycle.
5. Progress, parent-child relationships, events, and cancellation use one shared model.
6. A deterministic local artifact can be streamed, verified, and safely committed to cache.
7. Cancellation/failure cannot turn partial data into a valid final object.
8. Errors are structured and machine-readable.
9. Foundation tests are deterministic and offline.
10. Supported CI platforms build/test the foundation.
11. The public facade does not unnecessarily expose concrete HTTP/runtime/channel implementations.
12. Phase 1 can add Minecraft metadata and installation planning **without redesigning** operation,
    network, cache, errors, cancellation, or data-root ownership.

Item 12 is the architectural proof that Phase 0 succeeded.

---

## 50. Conditions That Block Phase 1

Phase 1 must not begin while any of these are unresolved:

- cancellation can race into false success;
- a partial download can appear as a committed artifact;
- cache identity is undefined;
- cache commit is not race-safe;
- provider/domain logic would require raw HTTP-client access;
- errors still require string parsing;
- data-root ownership is ambiguous;
- service composition is tied to a UI framework;
- crate dependency cycles exist;
- tests require live Mojang/public-network availability;
- multiple isolated Graphene roots cannot coexist;
- operation/event semantics are still being independently reinvented by callers.

These defects become much more expensive once real Minecraft installation logic depends on them.

---

## 51. Deferred Decisions

Deliberately defer until later phases provide concrete requirements:

- SQLite implementation/schema;
- account metadata repository;
- secret-store implementation;
- managed Java providers;
- full HTTP metadata cache semantics;
- advanced bandwidth scheduling;
- sophisticated cross-session resumable downloads;
- Minecraft process execution;
- provider registry;
- dynamic extension/plugin protocol;
- instance lock format;
- Graphene instance lockfile schema;
- archive extraction framework;
- installer transaction model beyond reusable safe-write primitives.

Phase 0 should leave room for these capabilities without inventing speculative abstractions with no
immediate consumer.

---

## 52. Architectural Invariants During Implementation

Every Phase 0 change must preserve:

1. `graphene-core` has no UI/provider/network/storage implementation dependency.
2. The root crate remains a facade.
3. Networking is shared infrastructure.
4. Provider concepts do not leak into `graphene-network`.
5. Storage owns Graphene-managed layout policy.
6. Platform owns OS-specific behavior.
7. Long-running work uses one operation model.
8. Cancellation has explicit semantics.
9. Verification happens before artifact commit.
10. Final cache objects are never partial.
11. Error codes are machine-readable.
12. Public APIs do not require Tauri/Slint.
13. Tests do not require public internet.
14. No global mutable engine singleton exists.
15. No Phase 1 Minecraft business logic is pulled into Phase 0.

---

## 53. Handoff to Phase 1

After Phase 0 passes, Phase 1 may activate:

```text
graphene-minecraft
graphene-instance
graphene-install
graphene-java
graphene-launch
provider adapter(s) required for official Minecraft metadata
```

Expected reuse:

```text
Minecraft metadata
      |
      v
Artifact declarations ------------------+
      |                                 |
      v                                 v
InstallPlan                     graphene-network
      |                                 |
      v                                 v
transaction/storage             verified cache object
      |
      v
instance materialization

Launch planning
      |
      +--> shared errors
      +--> shared operation model
      +--> graphene-platform
      +--> later Java resolution
```

If Phase 1 requires a second downloader, second cancellation mechanism, second error contract, or UI
callbacks inside the engine, the Phase 0 foundation should be corrected before Phase 1 proceeds.

---

## 54. Phase 0 North Star

Foundation architecture:

```text
                   Host application
                         |
                         v
                    graphene
                         |
                         v
                graphene-service
                 /      |       \
                /       |        \
               v        v         v
          operations  network   storage
               \        |         /
                \       |        /
                 +------v-------+
                        |
                    platform
                        |
                        v
                   graphene-core
```

Artifact acquisition:

```text
Artifact
   |
   v
DownloadRequest
   |
   v
Network policy
   |
   v
Temporary file
   |
   +--> progress/events
   |
   +--> cancellation
   |
   v
size/hash verification
   |
   v
safe cache commit
   |
   v
Verified artifact result
```

Everything implemented in Phase 0 must make these two flows more concrete while preserving the
project-wide rule:

> Stable domain contracts point inward; volatile infrastructure stays behind explicit boundaries.
