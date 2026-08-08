# Phase 1 API — Vanilla End-to-End

This document describes the public behavior implemented for Graphene Phase 1. The normative scope
and acceptance criteria remain in `PHASE_1_IMPLEMENTATION_PLAN.md`.

## Engine Composition

`Graphene::builder(data_root)` constructs an explicit, UI-independent engine. The builder accepts
Graphene-owned network configuration and an optional `MojangProviderConfig`. Production provider
defaults use the official-compatible Mojang endpoints over HTTPS. `MojangProviderConfig::fixture`
is the explicit test-only path that permits local HTTP and rewrites frozen `fixture.invalid` source
URLs to a deterministic local fixture origin.

The root facade exposes Graphene-owned values only. Provider DTOs, Reqwest responses, ZIP parser
internals, Tokio child handles, and filesystem publication primitives are private implementation
details.

## Minecraft Metadata

`graphene.minecraft().versions()` returns a cancellable `MinecraftManifestOperation`. Its result is
a normalized `VersionManifest` supporting explicit `MinecraftVersionId` selection. Version IDs are
never implicitly resolved from `latest` for deterministic installation requests.

Phase 1 normalization supports the Tier A model required by the implementation plan: version type,
parent-first inheritance, Mojang rules, OS/architecture/features, modern arguments, the supported
legacy quoted argument form, Maven coordinates, classpath libraries, native classifiers, assets,
logging configuration, Java requirements, and the provider-neutral `ResolvedMinecraft` model.
Inheritance depth and cycles are bounded and rejected structurally.

## Installation

`InstallRequest` contains a validated create-only `NewInstanceSpec` and an explicit
`MinecraftVersionId`.

`graphene.install().plan(request)` returns a cancellable `InstallPlanOperation`. Planning validates
the target before provider access, resolves official-compatible metadata through the provider
adapter, and produces a complete `InstallPlan` before committed-instance mutation. The plan contains
the client, classpath libraries, native archives, asset index and objects, logging configuration,
version metadata, materialization destinations, native extraction work, and a schema-versioned
provider-neutral receipt.

`graphene.install().execute(plan)` returns a cancellable `InstallExecutionOperation`. The executor:

1. validates the complete plan;
2. acquires every immutable artifact through the installer-owned `ArtifactAcquirer` port;
3. uses the service adapter to delegate that port to the Phase 0 verified acquisition/cache path;
4. re-verifies existing shared immutable materializations before reuse;
5. creates an isolated staging instance;
6. materializes instance content and safely extracts natives;
7. writes `instance.json` and `.graphene/install.json`;
8. validates staged state;
9. seals cancellation immediately before publication; and
10. publishes the create-only target with no-replace semantics.

A failure or cancellation before the publication seal does not create a valid-looking committed
target. Verified shared immutable cache/materialization state acquired before cancellation may
remain reusable.

The durable schemas begin at version `1` (`INSTANCE_SCHEMA_VERSION`,
`INSTALL_RECEIPT_SCHEMA_VERSION`, and `INSTALL_FORMAT_VERSION`). Persisted paths are managed
relative paths; provider DTOs, absolute host roots, and launch-session secrets are not persisted.

## Java Runtime

Phase 1 provides local Java only. `discover_java_candidates`, `probe_java`, `select_java`, and
`JavaService::select_for_instance` discover/probe/select deterministic compatible runtimes from an
explicit override, environment candidates, and bounded platform candidates. Probes spawn the Java
executable directly, never through a shell, with bounded timeout and output capture.

Java 8 `1.8` version strings and modern integer-major versions are normalized. Vendor and
architecture values are Graphene-owned. Candidate deduplication and selection ordering are
deterministic. No managed Java download/install behavior is implemented in Phase 1.

## Launch Planning

`LaunchSession` is ephemeral. Secret-bearing fields use `SensitiveString`; their `Debug` and
`Display` representations are redacted and they are not serializable.

`LaunchRequest` identifies a committed instance and carries the ephemeral session, optional
resolution, optional explicit Java override, launch features, and bounded extra arguments.

`graphene.launch().plan(request)` selects Java locally and reconstructs launch state only from
committed local files. `graphene.launch().plan_with_java(request, runtime)` performs the same
offline reconstruction with a preselected runtime. Neither API performs metadata network access.

`LaunchPlan` contains direct argv elements, deterministic classpath order, working directory,
natives path, platform separator, environment delta, and main class. Classpath materialization is
deferred until the process boundary so the platform separator is applied exactly once. Tier A
planning rejects unresolved `${...}` placeholders. Secret placeholders remain classified as secret
argv values rather than flattened into inspectable strings.

`LaunchPlan::redacted_snapshot(data_root)` returns a deterministic `RedactedLaunchPlan` with managed
roots normalized and secret argv values replaced by `<secret>`.

## Process Runtime

`graphene.launch().execute(plan)` validates the plan and directly spawns the Java executable with
argv. No shell command is constructed. `RunningGame` exposes:

- process ID;
- one bounded `GameEventStream` for start, stdout, stderr, exit, and failure events;
- deterministic lossy text alongside bounded raw output chunks;
- `dropped_output_count()` for host-facing output dropped under backpressure;
- `wait()` for the terminal `GameExit`; and
- `kill()` for explicit termination.

Stdout and stderr are drained independently of the host event consumer. A slow host cannot block
child pipe draining; output events are dropped when the bounded queue is saturated while terminal
capacity is reserved.

## Operations and Cancellation

Long-running metadata and install APIs use the existing Phase 0 operation model. Call
`operation()` on a prepared operation to obtain an `OperationHandle`, subscribe to structured
events, inspect state, or request cancellation. Cancellation remains idempotent and preserves the
Phase 0 terminal-state contract.

## Structured Errors

Phase 1 extends the existing `GrapheneError`/`ErrorCode` contract with stable machine-readable
families for Minecraft resolution (`MINECRAFT_*`), installation (`INSTALL_*`), Java (`JAVA_*`), and
launch/process execution (`LAUNCH_*`). Safe context can identify versions, instances, artifacts,
coordinates, Java majors, stages, and process exit information. Access tokens, credential-bearing
URLs, full secret argv, and arbitrary full environments are not error context.

## Phase Boundaries

Phase 1 intentionally does not activate authentication providers, loaders, managed Java downloads,
content/mod catalogs, modpacks, UI adapters, Tauri, Slint, or a dynamic plugin ABI. Phase 2 can
construct `LaunchSession` without changing argument logic, and later loader phases can continue to
produce a final `ResolvedMinecraft` without loader-specific branches in launch execution.
