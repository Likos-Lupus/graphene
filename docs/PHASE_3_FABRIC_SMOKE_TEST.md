# Phase 3 Fabric Loader Real Smoke Test

**Status: NOT RUN**  
**Pinned base:** Minecraft `1.21.1`  
**Pinned loader:** Fabric Loader `0.16.14`  
**Reason:** the current execution environment has no `cargo`/`rustc`, and the implementation could
therefore not be built into an executable Graphene smoke harness. Deterministic fixtures are not a
substitute for this real upstream/runtime smoke.

## Procedure

1. Use a clean Graphene data root and production HTTPS provider configuration.
2. Resolve exactly Minecraft `1.21.1` + Fabric Loader `0.16.14`; record that no moving selector
   remains in the plan.
3. Inspect the component graph: exactly `net.minecraft 1.21.1` plus
   `net.fabricmc.fabric-loader 0.16.14`, with no Forge-family component.
4. Inspect final `ResolvedMinecraft` and confirm Fabric contributes normalized main class,
   libraries, and structured arguments while Minecraft remains authoritative for base assets/client
   metadata.
5. Confirm no Fabric API artifact/component and no Java installer processor is present.
6. Inspect the deterministic component-aware `InstallPlan`, then install into a new instance.
7. Stop Graphene completely and restart with the same data root.
8. Disable loader-provider/network access.
9. Select a compatible Java runtime, create/use the normal `LaunchSession`, and build `LaunchPlan`
   only from committed state.
10. Launch and verify bounded/safe Fabric initialization evidence (loader identity/version) without
    copying account tokens or unbounded logs into the record.
11. Terminate the game through the normal process lifecycle.
12. Re-enable acquisition only if needed, create a second instance for the same exact pair, and
    verify ordinary immutable artifact cache reuse; Fabric must still have no processor-generated
    output path.

## Pass Criteria

A PASS requires actual upstream resolution, installation, process launch, Fabric initialization
proof, restart/offline launch planning, and absence of loader-provider access after commit. No such
execution occurred in this session.
