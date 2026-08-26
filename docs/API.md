# Graphene API

This document describes the current public behavior of the root `graphene` facade. It is organized
by capability rather than implementation chronology. Provider DTOs and infrastructure implementation
types are intentionally not part of the public contract.

## Engine construction

Create an engine around an explicit data root:

```rust,no_run
use graphene::Graphene;

# async fn example() -> Result<(), graphene::GrapheneError> {
let graphene = Graphene::builder("./graphene-data").build().await?;
# let _ = graphene;
# Ok(())
# }
```

`GrapheneBuilder` configures immutable network policy, operation event capacity, Mojang/loader
provider endpoints, optional Microsoft authentication, an injected `SecretStore`, and the managed
Java reference-provider configuration. Build validates configuration, initializes the data-root
layout, and composes one engine. Multiple engines may coexist with separate roots; Graphene does not
install a process-global async runtime or tracing subscriber.

The facade exposes services through `operations()`, `accounts()`, `artifacts()`, `minecraft()`,
`loaders()`, `install()`, `java()`, `launch()`, `instances()`, and `content()`.

## Instance engine and lifecycle

`graphene.instances()` provides `InstanceService` for inventory inspection, global and per-instance
configuration, lifecycle transactions (rename, clone, delete), structural/cryptographic
verification, and deterministic repair.

### Common repository & inventory

- `graphene.instances().list().await?`: returns deterministic `Vec<InstanceInventoryEntry>` sorted
  by `InstanceId`. Entries are classified as `Ready` (descriptor + receipt + lockfile valid),
  `Legacy` (valid descriptor + receipt, missing lockfile), or `Invalid` (malformed on-disk
  metadata).
- `graphene.instances().get(instance_id).await?`: loads and returns the committed
  `CommittedInstance` record.

### Configuration hierarchy & tri-state updates

Configuration evaluation precedence:
`built-in defaults < global defaults (config/instance-defaults.json) < per-instance explicit overrides (instances/<id>/.graphene/config.json)`

Explicit tri-state update semantics are modeled via `SettingUpdate<T>`:

- `SettingUpdate::Unchanged`: leaves existing setting unchanged.
- `SettingUpdate::Set(T)`: sets an explicit configuration override.
- `SettingUpdate::Inherit`: clears explicit override, resetting to inherit from higher precedence.

Methods:

- `global_defaults().await?` / `update_global_defaults(patch).await?`
- `effective_config(instance_id).await?` / `update_config(instance_id, patch).await?` (holds
  exclusive lease during update)

### Advisory lease model & concurrency

All instance operations synchronize across processes using OS-level advisory file locks (`fs2`)
backed by persistent carrier files (`instances/.locks/<id>.lock`). Lock carrier files are persistent
infrastructure and are never deleted on unlock.

- **Shared Lease (`InstanceSharedLease`)**: acquired during launch planning, process runtime
  execution (`RunningGame`), and instance verification scans.
- **Exclusive Lease (`InstanceExclusiveLease`)**: acquired during install staging/publication,
  config mutation, rename, clone destination/source staging, delete quarantine, and repair
  execution.
- Contention maps directly to a typed `ErrorCode::InstanceBusy` error.
- Multi-instance operations (e.g. clone) acquire leases in deterministic sorted `InstanceId` order
  to prevent deadlocks.

### Rename, clone, and delete transactions

- `rename(instance_id, display_name)`: updates only `instance.json` display metadata under an
  exclusive lease.
- `clone(instance_id, request)`: returns `InstanceCloneOperation` copying instance files into
  staging without following symlinks, rewriting instance identities in all Graphene-owned metadata
  documents, and publishing create-only.
- `delete(instance_id, options)`: returns `InstanceDeleteOperation` performing atomic directory
  rename into quarantine trash (`instances/.trash/<id>-<op_id>`) as the point-of-no-return commit
  boundary, followed by best-effort cleanup.

### Verification and deterministic repair

- `verify(instance_id, mode)`: returns `InstanceVerifyOperation` generating a `VerificationReport`
  with structured `VerificationFinding` entries.
    - `VerificationMode::Quick`: structural, schema, and file-size presence checks without computing
      cryptographic hashes on large files.
    - `VerificationMode::Full`: streams cryptographic SHA-1 and SHA-256 hashes for all
      Graphene-managed files declared in desired state.
- `plan_repair(instance_id, options)`: derives a deterministic, non-mutating `RepairPlan` with
  closed action vocabulary (`AcquireArtifact`, `RestoreSharedMaterialization`,
  `RestoreInstanceMaterialization`, `RebuildNativeState`, `RunPreparation`,
  `RestoreGeneratedOutput`, `RewriteInstallReceipt`, `FinalizeLockfileMigration`).
- `execute_repair(plan)`: returns `InstanceRepairOperation` executing the plan through shared
  install primitives under an exclusive lease. Stale plans whose base state fingerprint differs from
  current disk state are rejected with `ErrorCode::InstanceRepairPlanStale`. Successful repair
  converges to a no-op second plan.

## Content management and mod workflow

`graphene.content()` provides `ContentService` for offline local mod inspection, remote catalog
discovery, explicit file recognition, deterministic mutation planning, and crash-recoverable
execution.

### Local offline scanning & metadata

- `graphene.content().scan(instance_id, compute_hashes)`: returns `ContentScanOperation` scanning
  `.minecraft/mods` strictly offline without network requests.
    - Returns `LocalContentInventory` containing `LocalContentFile` items classified as
      `ManagedHealthy`,
      `ManagedDrifted`, `RecognizedUnmanaged`, `UnmanagedKnownMetadata`, `UnmanagedUnknown`,
      `Disabled`, or `Invalid`.
    - Parses `fabric.mod.json`, `META-INF/mods.toml`, `META-INF/neoforge.mods.toml`, and legacy
      manifests in a bounded, non-executing manner.
    - Distinguishes enabled (`.jar`) and disabled (`.jar.disabled`) files.
    - Computes streaming SHA-1, SHA-256, and normalized Murmur2 hashes when requested.
    - Surfaces duplicate enabled logical mod IDs across physical files via `duplicate_mod_ids`.
    - Computes `ContentInventoryFingerprint` for optimistic concurrency and stale detection.

### Remote catalog discovery & explicit recognition

- `graphene.content().search(provider_id, query)`: returns `ContentSearchOperation` querying
  projects on Modrinth or CurseForge with normalized pagination, loader, and game-version filtering.
- `graphene.content().project(project_ref)`: returns `ContentProjectOperation` with detailed
  metadata.
- `graphene.content().versions(project_ref, filters)`: returns `ContentVersionsOperation` with
  versions.
- `graphene.content().version(version_ref)`: returns `ContentVersionOperation` with exact version
  detail.
- `graphene.content().recognize(instance_id)`: returns `ContentRecognitionOperation` explicitly
  uploading local hashes/fingerprints to registered providers to recognize unmanaged local files.

### Mutation planning & execution

- `graphene.content().plan(request)`: returns `ContentPlanOperation` generating an inspectable,
  deterministic, non-mutating `ContentMutationPlan`.
    - Actions: `InstallExactVersion`, `AdoptRecognizedLocal`, `UpdateManaged`, `SetEnabled`,
      `RemoveManaged` (with reverse required dependency protection), and `RemoveLocalExact`.
    - Resolves required dependency closures deterministically, surfaces optional dependencies, and
      rejects incompatibilities.
    - Sanitizes filenames against traversal and case-fold collisions.
    - Pins exact artifact sources, sizes, and digests.
- `graphene.content().execute(plan)`: returns `ContentExecuteOperation` applying the plan under the
  exclusive instance lease.
    - Pre-acquires verified artifacts into the immutable cache before taking the lease.
    - Revalidates `InstanceStateFingerprint` and `ContentInventoryFingerprint` against disk state.
    - Stages files, records transaction state in `.graphene/content-journal.json`, quarantines old
      files, publishes new files, performs enable/disable renames, and atomically commits
      `lock.json` schema 2.
    - Interrupted operations recover idempotently on subsequent lease acquisition.
- `graphene.content().find_updates(instance_id, policy)` / `plan_updates(instance_id, policy)`:
  discovers and plans updates for all managed mods, converging to a no-op when unchanged.

## Operations, errors, and diagnostics

Long-running work exposes an `OperationHandle` with retained state/snapshot, cancellation, bounded
events, and terminal results. States are `Created`, `Queued`, `Running`, `Cancelling`, `Succeeded`,
`Failed`, and `Cancelled`. Terminal state is immutable. Cancellation is cooperative and a small safe
commit may seal cancellation at its documented point of no return.

`GrapheneError` exposes stable `ErrorCode`, broad `ErrorKind`, safe structured context, and
developer text. Hosts should branch on codes/kinds rather than parse messages. `Diagnostic` is
separate structured evidence for user-facing explanation.

## Network and artifact acquisition

`NetworkConfig`, `ProxyPolicy`, `RedirectPolicy`, and `RetryPolicy` configure the shared transport.
TLS validation remains enabled in production. Protocol response bodies are bounded; artifact bodies
stream to managed temporary files.

`Artifact` describes ordered sources, expected size, kind/cache policy, and SHA-1/SHA-256 integrity.
`graphene.artifacts().acquire(...)` returns an `ArtifactOperation` and ultimately a
`VerifiedArtifact`. Verifiable cache identity prefers SHA-256 and falls back to SHA-1. A transfer is
verified before publication; hash/size failure never publishes a valid cache object.

Normal tests may use explicit local HTTP fixture configuration. Production provider defaults use
HTTPS-compatible endpoints.

## Minecraft metadata

`graphene.minecraft().versions()` returns a cancellable normalized `VersionManifest`. Installation
uses an explicit `MinecraftVersionId`; moving `latest` metadata is informational rather than an
implicit install selector.

Normalized metadata covers parent inheritance, rules/features, modern and supported legacy
arguments, Maven libraries, native classifiers, assets, logging configuration, and Java
requirements. Inheritance depth/cycles and rule/path/metadata bounds fail closed.

## Loader discovery and resolution

`graphene.loaders()` exposes normalized loader version/resolution operations using `LoaderKind`,
`LoaderVersionSelector`, `LoaderVersionSummary`, and `ResolvedLoader`. Moving provider selectors are
resolved to an exact `LoaderVersion` before planning.

Fabric, Forge, and NeoForge adapters remain private implementation details behind the normalized
provider boundary. Their current compatibility and real-smoke status are documented in
[`LOADER_SUPPORT.md`](LOADER_SUPPORT.md).

Resolved loaders contribute exact component identity, a `MinecraftVersionPatch`, and optional
`ComponentPreparationRecipe`. The public component graph and patch models are Graphene-owned and are
not provider DTOs.

## Installation

### Vanilla request

`InstallRequest` contains a validated create-only `NewInstanceSpec` and an explicit Minecraft
version.

### Component-aware request

`ComponentInstallRequest` extends planning with a normalized loader request while preserving the
same transaction/executor path.

`graphene.install().plan(...)`/component-aware planning returns a deterministic `InstallPlan` before
committed-instance mutation. The plan describes immutable acquisition, materialization, native
extraction, loader preparation where applicable, and schema-versioned provider-neutral receipt
state.

`graphene.install().execute(plan)`:

1. validates the complete plan;
2. acquires immutable artifacts through the verified artifact pipeline;
3. re-verifies reusable shared materializations/generated outputs where required;
4. stages instance state under an operation-owned path;
5. safely extracts/materializes required files;
6. executes declared loader processors through the install-tool port when present;
7. writes and revalidates `instance.json` plus `.graphene/install.json`;
8. seals cancellation at the final safe-commit boundary; and
9. publishes the create-only target without replacing an existing instance.

Failure/cancellation before publication does not leave a valid-looking committed target. Reusable
verified shared state acquired earlier may remain.

## Persistence compatibility

`INSTANCE_SCHEMA_VERSION`, `INSTALL_RECEIPT_SCHEMA_VERSION`, `INSTALL_FORMAT_VERSION`, and
`MANAGED_RUNTIME_SCHEMA_VERSION` identify durable formats. Current install receipts explicitly
persist exact component identity. Legacy receipt schema 1 remains readable by synthesizing its
implicit Vanilla component in memory without destructively rewriting the file merely to launch.

Persisted paths are managed-relative. Provider DTOs, absolute host roots, auth secrets, and
transient processor staging data are not authoritative persisted state.

## Accounts and authentication

`AccountService` supports provider-neutral account inventory/lifecycle, offline account creation,
and Microsoft authentication when `MicrosoftAuthConfig` plus a suitable secure `SecretStore` are
provided.

Microsoft device authorization is represented as UI-independent interactions/session operations;
protocol credentials remain in `SensitiveString`/secret-store records rather than account metadata.
Refresh/entitlement/profile operations use the same provider-neutral account/session boundary.

The default builder does **not** silently persist secrets: without an injected usable secure store,
secret-persistence-dependent authenticated workflows fail explicitly.

## Java selection and managed runtimes

Graphene-owned Java values include `JavaRequirement`, `JavaRuntime`, `JavaArchitecture`, vendor/
image/platform descriptors, and managed runtime descriptors.

Local discovery/probe/selection is deterministic and direct-executable based. Java probes use
bounded process execution without a shell and normalize Java 8 and modern major-version formats.

`graphene.java().managed_runtimes()` lists committed managed runtimes.
`install_managed(requirement)`
and `ensure_for_instance(...)` are explicit side-effecting operations: they resolve a distribution
release, acquire a verified archive, extract to staging, locate/probe the Java executable, validate
a relative descriptor, and publish an immutable managed runtime. Selection may reuse a compatible
committed managed runtime.

## Launch planning and process lifecycle

`LaunchSession` is ephemeral; secret-bearing fields use `SensitiveString` and redact formatting.
`LaunchRequest` identifies a committed instance and carries the session, optional resolution/
explicit Java, features, and bounded extra arguments.

Launch planning reconstructs state from committed Graphene files rather than provider access. It
selects/uses Java, evaluates persisted rules/arguments, builds classpath/native/logging/assets
paths, and returns an inspectable `LaunchPlan`. Secret arguments remain classified so redacted plans
can be shown safely.

Execution launches Java directly with argv, not a shell command. `RunningGame`, `GameEventStream`,
and `GameExit` are Graphene-owned lifecycle abstractions; underlying Tokio child handles stay
private. Stdout/stderr are drained with bounded host-facing behavior so a slow consumer cannot
indefinitely block child pipes.

## API stability direction

The public contract is Graphene-owned types and capability services. Provider protocols, exact crate
file layouts, helper sequencing, transport/process handles, and internal DTOs are not compatibility
promises. Persisted schema changes require explicit migration/compatibility handling and regression
coverage.

## Modpacks

Modpack support is exposed through `Graphene::modpacks()` (`ModpackService`) and operates on
normalized, provider-neutral pack models. Format differences terminate at import; committed
instances are ordinary Graphene desired state.

### Operations

| Operation        | Input                                                        | Output                | Notes                                                                                               |
|------------------|--------------------------------------------------------------|-----------------------|-----------------------------------------------------------------------------------------------------|
| `inspect`        | `PackSource::{LocalFile, HttpsUrl}`                          | `PackInspection`      | Pins the source into the content-addressed cache first; all later steps read only pinned bytes.     |
| `plan_import`    | `ModpackImportRequest { snapshot, target, optional_policy }` | `ModpackImportPlan`   | Deterministic composite plan (install plan + origin + fingerprint); performs no committed mutation. |
| `execute_import` | `ModpackImportPlan`                                          | `CommittedInstance`   | One staged transaction: base runtime, managed files, seed layers, metadata commit.                  |
| `plan_export`    | `ModpackExportRequest`                                       | `ModpackExportPlan`   | Reads lockfile desired state, hashes seed selection, computes a staleness fingerprint.              |
| `execute_export` | `ModpackExportPlan`                                          | `ModpackExportResult` | Snapshot with stale checks, deterministic archive, self-validation, create-only publication.        |

Every operation is an operation object exposing `operation()` (handle for progress/cancellation)
and `await_result()`.

### Key types

- `PackSource` — local file or HTTPS URL import source; both become observed SHA-256/SHA-512
  snapshots in the managed cache before any parsing.
- `OptionalSelectionPolicy` (`RequiredOnly`, `IncludeAllOptional`, `Explicit`) — host-visible
  optional-file choice and part of plan identity.
- `ExportEmbeddingPolicy` (`ReferenceOnly`, `EmbedExplicit`) — explicit redistribution policy;
  embedding requires the host to name each destination.
- `ModpackExportRequest::with_seed` — selects user-mutable files relative to `.minecraft/` as seed
  payload; reserved launcher state is rejected.
- Diagnostics are typed pairs (e.g. `EXPORT_EMBEDDING_DECISION_REQUIRED`) on export plans; failures
  are structured `GrapheneError`s with stable codes such as `PACK_PLAN_STALE`,
  `PACK_EXPORT_STALE`, `PACK_SOURCE_TOO_LARGE`, `INSTALL_TARGET_EXISTS`.

### Supported formats

- Modrinth `.mrpack` v1 (mandatory SHA-1+SHA-512, override layering, URL host policy).
- CurseForge manifest (exact `(project_id, file_id)` resolution through the content provider
  boundary; embedded override mods promoted to managed content).
- Prism/MultiMC instances (standard components; foreign launch semantics fail explicitly).
- Graphene pack v1 (strict manifest, embedded objects at hash-derived paths, explicit seeds).
- Generic archives require an explicit runtime/root decision; malformed known formats cannot be
  bypassed by generic mode.

### Lockfile schema 3

`InstanceLockfile` schema version 3 adds optional bounded `pack_origin`
(format/name/version/source SHA-256/external ids). Schemas 1 and 2 remain readable without eager
rewrite; content mutations preserve the origin. Pack-managed artifacts are ordinary
`LockedArtifact` entries and mods are ordinary `LockedContentEntry` entries, so verify/repair need
no format branches.
