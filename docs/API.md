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
`loaders()`, `install()`, `java()`, and `launch()`.

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
