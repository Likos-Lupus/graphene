# ADR-0002: Select Phase 0 Foundation Dependencies

- **Status:** Accepted
- **Date:** 2026-08-07

## Context

Phase 0 needs a shared async HTTP transport with TLS, streaming, redirects, proxy policy,
cancellation, structured tracing, hashing, typed opaque IDs, serialization, and race-safe
cooperating file replacement. These dependencies sit below later provider and launcher-domain code,
so concrete types must remain behind Graphene-owned APIs.

## Decision

1. Use Tokio for async I/O, task scheduling, timers, synchronization, and test fixtures. Graphene
   does not create or own a process-global runtime.
2. Use Reqwest 0.12 with default features disabled, Rustls TLS, streaming, and system-proxy support.
   Graphene exposes its own network configuration and result types rather than Reqwest types.
3. Use `sha1` and `sha2` for incremental SHA-1/SHA-256 verification.
4. Use `uuid` only inside strongly typed `OperationId` and `ArtifactId` wrappers.
5. Use Serde for stable value/event serialization and the layout marker.
6. Use `tracing` as a facade only. Graphene never installs a subscriber.
7. Use `fs2` only for cooperating filesystem replacement locks. Storage policy remains outside the
   platform crate.
8. Use `tempfile` only in tests; production temporary paths are owned by the Graphene storage
   layout.

All dependency versions are declared at workspace level. Transport default features are disabled to
avoid an unused native TLS stack and unrelated codecs.

## Consequences

### Positive

- one reusable HTTP/TLS implementation serves future adapters;
- large artifact bodies can stream directly to managed temporary files;
- runtime, HTTP, channel, and filesystem-lock implementation types stay out of the root facade;
- tests can run against a deterministic local Tokio TCP fixture with no public service dependency;
- no hidden global runtime or tracing subscriber is introduced.

### Negative

- Tokio is the internal asynchronous execution dependency for Phase 0 service implementation;
- Reqwest/Rustls and Tokio contribute meaningful transitive dependency weight;
- `fs2` replacement locks coordinate Graphene writers but are not a general transaction system.

## Revisit Conditions

Revisit this ADR only if a selected dependency no longer supports the workspace MSRV/platform
matrix, introduces an unacceptable security or maintenance risk, or forces concrete implementation
types through Graphene's stable facade.
