# Contributing to Graphene

Graphene uses architecture rules as part of the contribution contract.

## Non-Negotiable Rules

1. Domain crates do not depend on UI frameworks.
2. Domain crates do not depend on concrete providers.
3. Provider DTOs do not cross adapter boundaries.
4. No global mutable singleton is required for normal library use.
5. Raw HTTP calls do not spread through business logic.
6. Business modules do not implement private download stacks.
7. Minecraft execution uses argv, not shell-string concatenation.
8. Public behavior does not depend on parsing human-readable error strings.
9. Secrets are not written to logs.
10. Installation and repair go through `InstallPlan`.
11. Launch execution goes through `LaunchPlan`.
12. Artifacts are verified when trustworthy hashes are available.
13. Long operations are cancellable.
14. Long operations expose unified progress.
15. Complex instance mutation must provide failure recovery.
16. Cache/database data is not the sole copy of user-owned instance state.
17. Public APIs do not expose provider DTOs.
18. Public APIs do not expose Tauri or Slint types.
19. New loaders extend component/provider abstractions first.
20. New content integrations extend `ContentProvider` first.
21. Pack formats normalize into one install model.
22. Repair reuses installation execution primitives.
23. Architectural exceptions require an ADR.

## Review Questions

Before merging a cross-crate change, verify:

- the owning bounded context is clear;
- dependency direction remains one-way;
- the change does not leak an adapter type inward;
- the API would still make sense with another UI host;
- the API would still make sense with another provider;
- tests cover the deterministic domain behavior.
