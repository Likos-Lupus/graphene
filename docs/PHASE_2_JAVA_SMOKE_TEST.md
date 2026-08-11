# Phase 2 Real Managed Java Smoke Test

This is the opt-in production-provider sign-off required by the Phase 2 implementation plan. It is
separate from deterministic local-provider/archive fixtures.

## Prerequisites

- a Graphene build produced by a verified Rust toolchain;
- network access to the configured Adoptium production API and returned HTTPS archive origin;
- a supported Linux, Windows, or macOS platform/architecture;
- an empty test data root with sufficient disk space.

## Procedure

1. Build Graphene with the normal quality gates passing.
2. Start with an empty data root and production `AdoptiumProviderConfig`.
3. Construct a supported `JavaRequirement` (for example the major required by a pinned Vanilla
   version).
4. Call `java().install_managed(requirement)` or an explicit `ensure_for_instance` path.
5. Observe stages for release resolution, artifact download/verification, extraction, probe, and
   commit.
6. Verify provider metadata selected a release with SHA-256 integrity and an HTTPS archive source.
7. Verify the committed descriptor is under `shared/runtimes/<managed-runtime-id>/runtime.json`,
   stores a relative executable path, and contains no absolute data-root path.
8. Verify the staged Java probe reports the expected major and architecture before publication.
9. Restart Graphene and verify `java().managed_runtimes()` discovers the committed runtime.
10. Repeat the explicit install/ensure request and verify the immutable committed runtime/cache is
    reused rather than replaced in place.
11. Use the returned existing `JavaRuntime` with the Phase 1 launch planner where a local installed
    fixture/real instance is available.
12. Record safe evidence: date, platform/architecture, required major, selected release identity,
    SHA-256, runtime ID, normalized probe result, and operation outcome.

## Execution Record for This Delivery

- Date: 2026-08-11
- Status: **NOT RUN**
- Reason: this sandbox has no Rust toolchain and cannot build the candidate; outbound provider
  access is also unavailable in the execution container. Frozen local fixtures were not treated as a
  production-provider smoke pass.
