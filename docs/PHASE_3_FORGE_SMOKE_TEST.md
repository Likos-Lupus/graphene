# Phase 3 Forge Real Smoke Test

**Status: NOT RUN**  
**Pinned base:** Minecraft `1.21.1`  
**Pinned loader:** Forge `52.1.0`  
**Reason:** the current execution environment has no `cargo`/`rustc`, so no executable Graphene
loader smoke could be built. Deterministic installer fixtures do not count as a real Forge smoke.

## Procedure

1. Use a clean Graphene data root and production HTTPS provider configuration.
2. Resolve exactly Minecraft `1.21.1` + Forge `52.1.0`; record the exact component/Maven installer
   identity and provider-supplied installer digest.
3. Acquire the installer through the existing verified `Artifact` path; confirm no installer bytes
   are parsed before verification.
4. Inspect the normalized component graph and final `ResolvedMinecraft`; the base version remains
   exactly `1.21.1`.
5. Inspect the detected installer schema and normalized `ComponentPreparationRecipe`: typed data,
   explicit dependencies, client-side processors, allowlisted placeholders, declared outputs, and
   installer/tool Java requirement.
6. Inspect the `InstallPlan` and verify there are no provider callbacks or execution-only staging
   paths in its identity.
7. Install and record which `JavaRuntime` was explicitly selected for each processor requirement.
8. Verify processors execute in synthetic staging, receive direct argv (no shell), and cannot write
   through a hardlink to the shared artifact cache.
9. Verify each declared generated output, its upstream hash when present, local SHA-256, provenance,
   and atomic canonical publication before instance commit.
10. Stop Graphene, restart with the same data root, and disable Forge/network access.
11. Select game Java, create/use the normal `LaunchSession`, build `LaunchPlan` from the receipt,
    and launch without Forge provider/installer access.
12. Record bounded/safe Forge initialization evidence, then terminate through normal process
    lifecycle.
13. Install the same exact pair into a second new instance and verify valid generated output is
    reused without rerunning its expensive producer when the reuse contract applies.
14. Corrupt a disposable copy of the reusable generated object/provenance in an isolated smoke data
    root and verify Graphene detects it and regenerates instead of trusting file existence.

## Pass Criteria

A PASS requires real upstream installer verification, processor execution with selected Java,
verified generated output/provenance, transactional commit, restart/offline launch, Forge
initialization proof, and generated-output reuse/regeneration evidence. No such execution occurred
in this session.
