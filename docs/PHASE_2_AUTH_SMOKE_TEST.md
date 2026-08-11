# Phase 2 Real Microsoft Authentication Smoke Test

This is the opt-in real-service sign-off required by the Phase 2 implementation plan. It is kept
separate from deterministic fixtures and must never record secret values.

## Prerequisites

- a Graphene build produced by a verified Rust toolchain;
- network access to the distributor-configured Microsoft identity, Xbox, and Minecraft services;
- a distributor-owned Microsoft application/client ID and tenant policy suitable for device
  authorization;
- explicit production `MicrosoftServiceEndpoints` configuration;
- a real secure `SecretStore` backend supplied by the host/distributor;
- a test Microsoft account that owns Minecraft: Java Edition and has a Minecraft profile;
- an empty test data root.

## Procedure

1. Build Graphene with the normal quality gates passing.
2. Configure Microsoft identity and service endpoints without printing client configuration or
   credentials to logs.
3. Configure a real secure credential backend; do not use `InMemorySecretStore` for this smoke.
4. Call `accounts().begin_microsoft_login()` and verify the host can display the typed device
   interaction without Graphene owning UI.
5. Complete authorization in the official user flow.
6. Observe structured stages through entitlement/profile validation and account persistence.
7. Verify the resulting public account is `Ready` and `config/accounts/<id>.json` contains no token,
   device code, user code, email, or unrelated provider claims.
8. Restart the engine using the same data root and secure credential backend.
9. Call `accounts().launch_session(account_id)` and verify refresh completes without host-side
   OAuth, Xbox, XSTS, or Minecraft protocol logic.
10. Use the resulting existing `LaunchSession` with the Phase 1 launch path in an authorized test
    launch where prerequisites permit.
11. Exercise account removal and confirm both public metadata and the secure credential record are
    removed.
12. Capture only safe evidence: date, platform, Graphene revision/delta, operation stage names,
    account ID, Minecraft UUID/display name if appropriate, and pass/fail outcome. Never record
    tokens or device/user codes.

## Execution Record for This Delivery

- Date: 2026-08-11
- Status: **NOT RUN**
- Reason: this sandbox has no Rust toolchain and cannot build the candidate. It also has no
  distributor-owned Microsoft application registration, production service configuration, real
  secure credential-vault backend, or real test credentials. Deterministic fixtures were not
  substituted for this real smoke.
