# Phase 3 NeoForge Real Smoke Test

**Status: NOT RUN**  
**Pinned base:** Minecraft `1.21.1`  
**Pinned loader:** NeoForge `21.1.200`  
**Reason:** the current execution environment has no `cargo`/`rustc`, so no executable Graphene
loader smoke could be built. Deterministic installer fixtures do not count as a real NeoForge smoke.

## Procedure

1. Use a clean Graphene data root and production HTTPS NeoForge Maven configuration.
2. Resolve exactly Minecraft `1.21.1` + NeoForge `21.1.200` from the normalized version API and
   record the exact component identity.
3. Resolve the official installer artifact and SHA-256 sidecar; acquire it through Graphene's
   existing verified artifact pipeline before parsing.
4. Inspect the normalized graph, `MinecraftVersionPatch`, and final `ResolvedMinecraft`; no
   synthetic Minecraft version ID may be the source of truth.
5. Inspect the common `ComponentPreparationRecipe`, including typed data, explicit processor
   dependencies, client-side filtering, tool Java requirement, and declared generated outputs.
6. Inspect the deterministic component-aware `InstallPlan`, then install into a new instance.
7. Verify selected processor Java comes through the Phase 2 Java boundary and execution remains in
   synthetic staging with direct argv and bounded output/timeout/cancellation behavior.
8. Verify every generated output and provenance record before canonical publication and final
   instance commit.
9. Stop Graphene, restart with the same data root, and make NeoForge/provider/network access
   unavailable.
10. Select game Java, create/use the existing `LaunchSession`, build `LaunchPlan` only from
    committed normalized state, and launch.
11. Record bounded/safe NeoForge initialization evidence, then terminate normally.
12. Install a second instance for the same exact pair and verify valid shared generated-output
    reuse; in an isolated disposable data root also verify corrupt output is rejected/regenerated.

## Pass Criteria

A PASS requires real upstream exact discovery/checksum acquisition, verified installer parsing,
processor execution, generated-output validation/publication, restart/offline launch, NeoForge
initialization proof, and reuse evidence. No such execution occurred in this session.
