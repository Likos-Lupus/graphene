# Phase 1 Real Vanilla Smoke Test

This is the manual/opt-in sign-off procedure required by the Phase 1 implementation plan. It is
separate from deterministic CI because it intentionally contacts official infrastructure and may
require an externally obtained authenticated launch session.

## Procedure

1. Use a new empty Graphene data root.
2. Configure production provider defaults and select an explicit pinned Tier A Vanilla version; do
   not use `latest` as the acceptance identity.
3. Fetch the official-compatible version manifest and resolve the pinned version.
4. Produce and inspect the deterministic `InstallPlan` before execution.
5. Execute installation and verify the committed instance appears only after successful completion.
6. Verify client, libraries, assets, logging metadata, native extraction, and shared/cache
   materializations.
7. Discover or explicitly provide a compatible local Java runtime and inspect the normalized probe
   result.
8. Supply an externally obtained ephemeral `LaunchSession` when required by the selected client.
9. Reconstruct a redacted `LaunchPlan` from the committed local metadata with network access
   disabled; confirm there are no unresolved placeholders or secrets in the exported plan.
10. Launch through Graphene and observe PID, start, stdout/stderr, and terminal lifecycle.
11. Close or kill the process through Graphene and verify `wait()` completes.
12. Install the same pinned version into a second create-only instance and confirm verified
    immutable artifacts are reused rather than downloaded again.
13. Record platform/architecture, pinned version, Java major/vendor/architecture, relevant Graphene
    revision/delta, instance IDs, observed lifecycle, and any deviations.

## Execution Record for This Delivery

- Date: 2026-08-08
- Status: **NOT RUN**
- Reason: the execution sandbox has no Rust toolchain and cannot reach the Rust distribution host;
  therefore the engine cannot be built here. The real official-network/authenticated smoke test was
  not fabricated or substituted with the deterministic fixture suite.
