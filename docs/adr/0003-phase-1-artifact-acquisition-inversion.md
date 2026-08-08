# ADR-0003: Invert Phase 1 Artifact Acquisition at the Installer Boundary

- **Status:** Accepted
- **Date:** 2026-08-08

## Context

Phase 0 owns the verified HTTP/download/cache pipeline in service composition, while Phase 1
installation must reuse that pipeline without introducing the forbidden dependency
`graphene-install -> graphene-service` or leaking Reqwest/provider DTOs into the installer.

## Decision

`graphene-install` owns the provider-neutral `ArtifactAcquirer` port. It accepts an already resolved
Graphene `Artifact` plus an operation handle and returns only Graphene-owned acquisition values.
`graphene-service` owns the concrete adapter and delegates each acquisition to the existing Phase 0
`ArtifactService`.

Provider metadata acquisition uses the same composition principle: `graphene-providers` requests a
resolved metadata `Artifact` through a narrow adapter implemented by service, rather than creating a
second downloader.

## Consequences

- one verified cache/download implementation remains authoritative;
- installer/provider crates do not depend on service or Reqwest;
- integrity and cancellation behavior from Phase 0 is reused rather than copied;
- later content/pack installers can reuse the same domain-side acquisition concept;
- service owns composition code but not install-domain algorithms.

## Revisit Conditions

Revisit only if the stable artifact contract itself changes or a future acquisition backend cannot
be represented without leaking provider/transport implementation types across the domain boundary.
