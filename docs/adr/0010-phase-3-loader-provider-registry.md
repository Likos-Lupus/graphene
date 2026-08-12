# ADR-0010: Resolve Loader Adapters Through a Provider Registry

- **Status:** Accepted for Phase 3 implementation candidate
- **Date:** 2026-08-12

## Context

Fabric, Forge, and NeoForge have distinct discovery/version/integrity protocols, but service and
install orchestration must not grow one permanent branch/pipeline per loader. Provider DTOs also
must not become stable service API.

## Decision

`graphene-providers` exposes the Graphene-facing `LoaderProvider` port and
`LoaderProviderRegistry`, while each adapter keeps its raw DTO modules private.

- Providers expose normalized version summaries, exact resolution, capabilities, and verified
  installer normalization where applicable.
- Request-time moving selectors (`LatestStable`, `Recommended`) are provider-specific conveniences
  and resolve to an exact `LoaderVersion` before planning.
- Capability flags describe supported operations such as exact resolution, profile patching,
  installer archives/processors, recommendations, and checksum sidecars.
- `GrapheneBuilder` performs the small composition-time mapping by registering Fabric, Forge, and
  NeoForge adapters once.
- `LoaderService` asks the registry for a provider. Install execution and launch do not ask
  providers or branch on loader kind.

Forge's production version-list API is intentionally based on its official promotions feed: it lists
promoted latest/recommended versions rather than pretending to enumerate every historical release.
An explicit exact Forge selector validates the corresponding official Maven installer/checksum even
when that version is not promoted.

## Consequences

Adding a future loader primarily requires another adapter plus registry registration and normalized
fixtures. Provider-specific ordering/version grammar remains outside the stable domain. Services can
report support/capabilities without importing private Fabric/Forge/NeoForge DTOs.
