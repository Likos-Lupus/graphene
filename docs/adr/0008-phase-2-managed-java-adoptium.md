# ADR-0008: Use Adoptium as the Replaceable Managed-Java Reference Provider

- **Status:** Accepted for Phase 2 implementation candidate
- **Date:** 2026-08-11

## Context

Phase 2 needs one real Java distribution adapter to prove that provider-neutral managed-runtime
models can resolve executable archives with trustworthy integrity without introducing another
downloader or installer-script path.

The Adoptium API v3 behavior and checksum-bearing release metadata were checked against the current
project-owned API cookbook and CI/download guidance:

- `https://github.com/adoptium/api.adoptium.net/blob/main/docs/cookbook.adoc`
- `https://adoptium.net/installation/ci-scripts`

Adoptium provides Eclipse Temurin JDK/JRE archive metadata across the Phase 2 desktop platform
matrix and exposes SHA-256 package checksums.

## Decision

Implement `AdoptiumProvider` in `graphene-providers` behind
`graphene_java::JavaDistributionProvider`.

- Domain request/release/runtime/install-plan models remain in `graphene-java`.
- Provider DTOs remain private.
- Production API and archive URLs require HTTPS; loopback HTTP is available only through explicit
  fixture configuration.
- Resolution uses the v3 latest-assets API for the requested major, HotSpot VM, platform,
  architecture, and image.
- A preferred JRE may fall back to a JDK for the same requirement when no JRE release is returned.
- SHA-256 is mandatory. Missing/invalid checksums are rejected.
- Only ZIP and tar.gz archives are accepted.
- The resolved archive becomes a normal Graphene `Artifact`; `ArtifactService` owns acquisition,
  cache, retries, integrity verification, and duplicate download gates.
- Runtime identity is immutable and derived from the verified release checksum for this reference
  provider.

## Consequences

- adding a second Java distribution provider requires another adapter implementing the same port,
  not changes to `JavaService` selection/install policy;
- Graphene never invokes vendor MSI/PKG/DEB/RPM installers or post-install scripts;
- archive security and staged probe/commit remain Graphene-owned rather than provider-owned;
- a newer provider patch creates a new managed identity instead of overwriting an existing runtime.

## Revisit Conditions

Revisit provider selection or identity derivation if additional trustworthy providers require richer
provenance/version identity. The provider-neutral `ManagedJavaRelease` and existing `JavaRuntime`
convergence should remain stable.
