# Phase 3 API — Loader Components

This document describes the Phase 3 loader-component behavior implemented in this worktree. The
normative scope and exit criteria remain in `PHASE_3_IMPLEMENTATION_PLAN.md`. This worktree is a
**Phase 3 implementation candidate**, not release sign-off: the current execution environment has no
Rust toolchain, the real Fabric/Forge/NeoForge smoke procedures have not been executed, and the
prior Phase 1/2 real smoke records remain `NOT RUN`.

## Component Domain

`graphene-minecraft` owns the provider-neutral component model:

- `ComponentUid` and `ComponentVersion` are bounded validated opaque strings;
- `ComponentKind` distinguishes Minecraft, primary loader, and auxiliary components;
- `ComponentDescriptor` carries exact requirements, conflicts, and deterministic order;
- `ComponentGraph` validates exactly one Minecraft base, at most one primary loader, uniqueness,
  requirements, conflicts, node/edge bounds, cycles, and deterministic topological order;
- `ResolvedComponent` carries exact identity plus safe provider provenance.

Vanilla resolution now includes an exact `net.minecraft <base-version>` component. Loader installs
add exactly one primary component (`net.fabricmc.fabric-loader`, `net.minecraftforge.forge`, or
`net.neoforged.neoforge`) without changing the base Minecraft version string.

## Metadata Composition

`MinecraftVersionPatch` is provider-neutral. A loader provider may contribute a main-class override,
libraries, JVM/game arguments, a compatible Java requirement, logging metadata, and component
provenance. `compose_minecraft` applies patches in component order and returns the ordinary
`ResolvedMinecraft` consumed by the existing install/launch pipeline.

Library replacement uses the existing normalized Maven identity rather than provider JSON. Scalar
fields preserve the current value when absent and replace it when explicitly supplied. JVM and game
arguments remain tokenized and append in deterministic component order. Conflicting Java
requirements fail before installation planning.

## Loader Selection

The facade exports:

- `LoaderKind::{Fabric, Forge, NeoForge}`;
- `LoaderVersion` and `LoaderVersionSummary`;
- `LoaderVersionSelector::{Exact, LatestStable, Recommended}`;
- `LoaderSelection`;
- `LoaderSupport::{Supported, MetadataOnly, Unsupported}`;
- `LoaderProviderCapabilities` and normalized loader diagnostics.

`Graphene::loaders()` returns `LoaderService`. Its version-list and resolve operations use the
provider registry and expose only normalized Graphene types. A moving selector is resolved to an
exact `LoaderVersion` before a component-aware install plan is created.

Provider-specific version ordering/recommendation remains inside each adapter. There is no global
SemVer comparator for loader versions.

## Provider Registry and Configuration

`GrapheneBuilder` registers Fabric, Forge, and NeoForge providers in one
`LoaderProviderRegistry`. The service layer selects a provider through that registry; downstream
install and launch code do not branch on loader families.

Builder overrides are available through:

- `fabric_provider(FabricProviderConfig)`;
- `forge_provider(ForgeProviderConfig)`;
- `neoforge_provider(NeoForgeProviderConfig)`.

Production configurations use HTTPS official-compatible endpoints. Explicit fixture constructors
permit local HTTP for deterministic tests without globally changing production origins.

### Fabric

The Fabric adapter uses Fabric Meta for loader lists and exact Minecraft+loader profiles. Private
Fabric DTOs normalize to an exact component, metadata patch, and ordinary verified libraries. Inline
hashes are used when supplied; otherwise the adapter reads the corresponding official Maven SHA-1
sidecar. Normal Fabric installation has no processor recipe. Fabric API is not resolved or
installed.

### Forge

The Forge adapter uses the official promotions feed for `latest`/`recommended` version-list
conveniences. The list is intentionally a promotions list, not an enumeration of every historical
Forge release. An exact request does not need to be promoted: Graphene constructs the official Maven
installer identity and requires its SHA-1 sidecar before acquisition.

After the installer has been acquired through the existing verified artifact pipeline, the provider
performs bounded installer-JAR inspection. Modern processor profiles normalize into the common
preparation recipe. Legacy launcher-profile installers are explicitly classified as metadata-only in
this candidate rather than being guessed by fallback parsing.

### NeoForge

The NeoForge adapter uses the official Maven versions API, keeps NeoForge-specific version
prefiltering inside the adapter, and requires the installer SHA-256 sidecar. The verified installer
is then normalized through the same provider-neutral Forge-family preparation model while retaining
a separate NeoForge provider adapter and component identity.

## Component-Aware Installation

The Phase 1 `InstallRequest` remains the Vanilla convenience path. Phase 3 adds
`ComponentInstallRequest`, containing the base Minecraft request plus one loader selection.
Vanilla-only installs continue to use the existing `InstallRequest`.

`InstallService::plan_components` performs, before returning an executable plan:

1. base Minecraft resolution;
2. exact loader resolution;
3. verified installer acquisition when required for profile normalization;
4. component-graph construction and validation;
5. patch composition into final `ResolvedMinecraft`;
6. preparation normalization;
7. complete artifact/generated-output enumeration and managed-path validation;
8. deterministic `InstallPlan` construction.

The executor never calls a loader provider. `INSTALL_PLAN_VERSION` is now `2`; Vanilla remains
representable and receives an empty preparation recipe.

## Preparation and Processor Port

Forge-family providers normalize verified installer data into `ComponentPreparationRecipe`:

- verified installer artifact;
- explicit input artifacts and embedded installer inputs;
- typed data values;
- ordered processor steps;
- declared generated outputs;
- optional installer/tool Java requirement when declared by the normalized upstream profile.

Raw installer placeholder syntax does not cross into execution. Processor arguments are structured
`PreparationArgument` values composed from literals and an explicit placeholder allowlist.

`graphene-install` depends on `InstallToolRunner`, not on services or a Java distribution provider.
The port first selects a `SelectedToolJava` for the normalized requirement, then runs a
`ToolRunRequest` containing the selected Java executable, explicit main class, executable JAR,
classpath, argv, managed working directory, timeout, and cancellation token.

`graphene-service` supplies the concrete runner using the existing Phase 2 Java boundary. It spawns
the selected Java executable directly, clears the inherited environment except for a small
OS/runtime allowlist, drains stdout/stderr into bounded buffers, and enforces cancellation and
timeout. No shell or installer-directed native/script execution is supported.

## Generated Outputs and Reuse

Processors run against a private synthetic staging tree. Verified cache inputs are copied into
staging; writable hardlinks to shared immutable cache objects are not used. Only declared outputs
can be published.

`GeneratedOutput` records producer, staged/final managed paths, publication scope, optional expected
size/digest, and input identity. Publication verifies declared SHA-1/SHA-256 where present, computes
a local SHA-256, atomically publishes shared immutable output, and writes bounded provenance. A
locally-computed digest is explicitly recorded as locally derived rather than upstream
authenticated.

A reusable generated output must match the managed destination, exact component/provider,
producer/input identity, embedded-input digests, declared integrity, size, and computed digest.
Existence alone never permits reuse. Reused output is copied back into private staging when a later
processor consumes it.

## Receipt Schema and Offline Launch

`INSTALL_RECEIPT_SCHEMA_VERSION` is `2`. `InstallReceipt` now persists exact components with kind,
UID, exact version, provider, and safe provenance detail in addition to final normalized launch
metadata.

Schema-1 Phase 1 Vanilla receipts are accepted in memory as an implicit component set containing
`net.minecraft <resolved-version>`. They are not destructively rewritten merely to launch.

Launch remains provider-neutral: committed receipt state, a selected `JavaRuntime`, and a
`LaunchSession` are sufficient to build the existing `LaunchPlan`. `graphene-launch` has no Fabric,
Forge, NeoForge, installer, or provider dependency.

## Support Classification

Support is finalized only after exact release/profile information is available. Fabric profile
resolution can report `Supported` directly. Forge/NeoForge exact resolution initially reports
`MetadataOnly` until the verified installer has been normalized; a known supported modern processor
profile becomes `Supported`, while legacy/unknown/unsupported behavior remains explicitly limited.

The evidence-based family/release status is maintained in `PHASE_3_SUPPORT_MATRIX.md`.
