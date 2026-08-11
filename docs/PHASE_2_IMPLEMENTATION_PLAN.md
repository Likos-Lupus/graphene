# Graphene Phase 2 — Detailed Authentication and Managed Java Implementation Plan

**Status:** Approved implementation plan  
**Phase:** 2 — Authentication and Managed Java  
**Project version baseline:** 0.1.x  
**Primary implementation language:** Rust 2024  
**Implementation state:** Documentation only; this revision contains no Phase 2 Rust
implementation  
**Normative parent documents:** `PROJECT_SPECIFICATION.md`, `ARCHITECTURE.md`,
`SCOPE_AND_BOUNDARIES.md`, `ROADMAP.md`, `PHASE_0_IMPLEMENTATION_PLAN.md`,
`PHASE_0_API.md`, `PHASE_0_SECURITY_REVIEW.md`, `PHASE_1_IMPLEMENTATION_PLAN.md`,
`PHASE_1_API.md`, `PHASE_1_SECURITY_REVIEW.md`, `PHASE_1_SMOKE_TEST.md`

---

## 1. Purpose

Phase 2 activates two major capabilities that Phase 1 deliberately left outside the Vanilla
install-to-launch proof:

1. persistent account identity and authentication;
2. launcher-managed Java runtime acquisition and selection.

The phase must integrate both capabilities into the existing Phase 1 contracts without redesigning
Minecraft resolution, installation, launch argument construction, or process execution.

The central architectural requirement is:

```text
Account
  |
  v
Authentication / refresh
  |
  v
LaunchSession
  |
  +------------------------------+
                                 |
Managed Java                    |
  |                              |
  v                              |
JavaRuntime ---------------------+
                                 |
                                 v
                         Existing LaunchPlan
                                 |
                                 v
                         Existing Process Path
```

Phase 2 is successful only if authentication and managed Java behave as replaceable upstream
suppliers to already-stable Phase 1 launch contracts.

Phase 2 must not turn `graphene-service` into an authentication implementation crate, must not put
provider protocol DTOs into domain crates, and must not place large new implementations directly in
crate-root `lib.rs` files.

---

## 2. Phase 2 Objective

The primary engineering objective is:

> Graphene can persist public account metadata separately from secrets, create and manage offline
> accounts, authenticate a Microsoft-backed Minecraft account through a UI-independent interactive
> flow, securely persist only the minimum refresh credential required for future authentication,
> refresh sessions without host-side protocol logic, verify Minecraft entitlement/profile state,
> convert both Microsoft and offline accounts into the existing ephemeral `LaunchSession`, discover
> or install a compatible launcher-managed Java runtime through a provider-neutral runtime model,
> feed the resulting `JavaRuntime` into the existing Phase 1 launch pipeline, and return structured
> compatibility/security diagnostics without exposing secrets or provider DTOs.

Phase 2 should establish durable contracts for:

- `AccountId`;
- `Account`;
- `AccountKind`;
- `AccountProfile`;
- `AccountState`;
- `AccountRepository`;
- `AuthProvider`;
- `AuthInteraction`;
- `AuthSession`;
- `MicrosoftAuthConfig`;
- `OfflineAccountSpec`;
- `SecretStore`;
- `SecretRecordKey`;
- `JavaDistributionProvider`;
- `ManagedJavaRequest`;
- `ManagedJavaRelease`;
- `ManagedJavaRuntime`;
- `ManagedJavaInstallPlan`;
- `JavaDiagnostic`;
- account-to-`LaunchSession` conversion;
- managed-runtime-to-`JavaRuntime` conversion.

These contracts must remain usable when later phases add loaders, richer instance configuration,
third-party authentication, content providers, modpacks, and dedicated diagnostics orchestration.

---

## 3. Implementation Gate from Phase 1

Phase 2 source implementation must not be merged on top of an unverified Phase 1 baseline.

Before Phase 2 implementation is accepted, the Phase 1 baseline must have executable evidence for:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
python scripts/check_architecture.py
```

The Phase 1 real Vanilla smoke procedure must also be completed and recorded.

At the time this document is written, the existing `PHASE_1_IMPLEMENTATION_PLAN.md` still records
many exit-checklist items as unchecked and `PHASE_1_SMOKE_TEST.md` records the real smoke test as
not run. This document therefore defines the Phase 2 implementation contract but does **not**
declare Phase 1 complete.

Phase 2 must preserve all Phase 0/1 invariants, including:

- explicit per-engine data roots;
- no global mutable engine singleton;
- no UI-framework dependency;
- no provider DTO leakage;
- no raw provider HTTP in domain/service crates;
- bounded network concurrency and response sizes;
- shared artifact acquisition and integrity verification;
- cancellable long-running operations;
- deterministic plans;
- structured errors;
- safe managed paths;
- direct process execution without shell construction;
- redacted secrets;
- offline launch planning after committed state exists;
- crate roots as thin public facades.

If Phase 2 requires changing a Phase 1 public contract, the change must be explicit, documented,
tested, and justified by an ADR rather than introduced as an incidental implementation shortcut.

---

## 4. Phase 2 Success Statement

Phase 2 is successful when all of the following are demonstrably true:

1. `graphene-auth` is activated as a bounded-context crate.
2. `AccountId` is a stable Graphene-owned typed identifier.
3. Public account metadata is persisted independently from secret credentials.
4. Offline accounts can be created, listed, read, renamed where allowed, and removed.
5. Microsoft authentication is exposed through a UI-independent interaction model.
6. The required interactive flow can be completed without Tauri, Slint, or browser-window types
   entering backend crates.
7. Microsoft provider DTOs remain private to `graphene-providers`.
8. OAuth/Xbox/Minecraft service access tokens remain ephemeral.
9. Only the minimum refresh credential required for future Microsoft authentication is persisted.
10. Persistent secrets are stored through a secure secret-store abstraction.
11. Plaintext credential files under the Graphene data root are forbidden.
12. Secret-store unavailability produces a structured failure rather than silently falling back to
    plaintext.
13. Refresh-token rotation is persisted safely.
14. Invalid/revoked refresh credentials transition the account into a reauthentication-required
    state without deleting public account metadata.
15. Minecraft entitlement/profile verification is part of Microsoft account activation.
16. Both Microsoft and offline accounts can produce the existing Phase 1 `LaunchSession`.
17. `graphene-launch` does not learn Microsoft OAuth, Xbox, entitlement, or secret-store logic.
18. Phase 1 launch argument construction remains provider-neutral.
19. `graphene-java` gains managed-runtime domain models without embedding a concrete distribution
    provider.
20. A concrete Phase 2 Java distribution adapter can enumerate/download a compatible release.
21. Managed Java archives are represented as Graphene `Artifact` values.
22. Managed Java downloads use the existing verified artifact acquisition path.
23. Managed Java archive extraction is bounded and traversal-safe.
24. Managed runtimes are installed transactionally under `shared/runtimes/`.
25. Runtime installation is not considered committed until the extracted Java executable probes
    successfully.
26. A committed managed runtime normalizes into the existing `JavaRuntime`.
27. Java selection can deterministically prefer explicit, compatible local, or compatible managed
    runtimes according to a documented policy.
28. Missing/incompatible Java returns structured diagnostics as well as structured errors where
    appropriate.
29. Account login/refresh and managed Java installation expose unified operations, progress, and
    cancellation.
30. Secrets do not appear in logs, `Debug`, errors, snapshots, exported diagnostics, operation
    events, filenames, URLs recorded in context, or persisted account metadata.
31. Deterministic tests do not require real Microsoft credentials, public Microsoft/Xbox/Minecraft
    services, or public Java-distribution infrastructure.
32. A separate opt-in real Microsoft authentication smoke procedure is documented.
33. A separate opt-in managed Java smoke procedure is documented.
34. Windows, Linux, and macOS CI cover the portable domain and storage behavior.
35. The architecture checker prevents Phase 2 dependency reversals and oversized crate-root
    implementations.

---

## 5. Scope

### 5.1 In Scope

Phase 2 includes:

1. Activating `graphene-auth`.
2. Adding `AccountId` to the stable identifier layer.
3. Defining provider-neutral account models.
4. Defining account repository behavior.
5. Persisting public account metadata.
6. Offline account creation.
7. Offline account launch-session generation.
8. Microsoft authentication configuration.
9. Microsoft interactive authentication.
10. A typed device-code interaction contract as the required Phase 2 baseline flow.
11. Cancellation-aware device authorization polling.
12. Microsoft token exchange normalization.
13. Xbox authentication stages required to reach Minecraft services.
14. Minecraft service login.
15. Minecraft entitlement verification.
16. Minecraft profile retrieval.
17. Microsoft account activation only after profile/entitlement validation.
18. Refresh-token persistence.
19. Refresh-token rotation.
20. Session refresh.
21. Reauthentication-required account state.
22. Account removal and secret deletion.
23. Secret-store abstraction.
24. OS keyring integration boundary.
25. Secure encrypted-file fallback design and implementation when explicitly configured.
26. In-memory secret-store implementation for deterministic tests only.
27. Account service APIs.
28. Account operation progress/events.
29. Account-to-`LaunchSession` conversion.
30. Activation of managed-Java models inside `graphene-java`.
31. Java distribution provider interface.
32. A concrete reference managed-Java provider.
33. Managed Java release selection.
34. Managed Java artifact normalization.
35. Managed Java installation request/plan/result models.
36. Reuse of the Phase 0 artifact acquisition pipeline.
37. Safe runtime archive extraction.
38. Transaction staging for runtimes.
39. Runtime descriptor persistence.
40. Runtime probe before commit.
41. Managed runtime inventory.
42. Managed runtime selection integration.
43. Java compatibility diagnostics.
44. Account/Java structured error expansion.
45. Phase 2 fixture servers and frozen provider fixtures.
46. Security regression tests.
47. Secret-redaction tests.
48. Cross-platform tests.
49. Documentation and smoke-test procedures.

### 5.2 Explicitly Out of Scope

Phase 2 must not implement:

- Fabric;
- Forge;
- NeoForge;
- Quilt;
- loader composition;
- third-party Yggdrasil;
- authlib-injector;
- Mojang legacy-account migration;
- Google/Apple/social login;
- account linking between unrelated providers;
- automatic account selection per instance;
- instance-level account override persistence;
- multi-instance mutation features from Phase 4;
- lockfile/repair expansion from Phase 4;
- Modrinth;
- CurseForge;
- mod management;
- modpacks;
- skin upload/edit APIs;
- cape management UI;
- a dedicated `graphene-diagnostics` crate;
- general launcher self-update;
- arbitrary plugin loading;
- cloud account synchronization;
- embedded browser/window implementations;
- storing Microsoft/Xbox/Minecraft access tokens on disk;
- storing any credential in plaintext JSON;
- silently inventing a Microsoft client ID;
- borrowing another launcher's OAuth application registration;
- silently executing downloaded installer programs;
- MSI/PKG/DEB/RPM installation through privileged system installers;
- arbitrary JDK vendor-specific post-install scripts.

Phase 2 may prepare extension points for these features, but it must not implement fake placeholder
behavior for later phases.

---

## 6. Product-Level Phase 2 Flow

### 6.1 Microsoft Account Flow

```text
Host
 |
 | begin Microsoft login
 v
AccountService
 |
 v
AuthProvider interface
 |
 v
Microsoft provider adapter
 |
 +--> request interactive challenge
 |        |
 |        v
 |   AuthInteraction::DeviceCode
 |        |
 |        +--> host presents URL/code
 |        |
 |        v
 |   polling / cancellation
 |
 v
Microsoft identity token
 |
 v
Xbox user authentication
 |
 v
XSTS authorization
 |
 v
Minecraft services login
 |
 +--> entitlement check
 |
 +--> profile fetch
 |
 v
Normalized AuthSession
 |
 +--> persist public Account metadata
 |
 +--> persist refresh credential in SecretStore
 |
 v
Account ready
 |
 v
AccountService::launch_session(account_id)
 |
 v
existing LaunchSession
 |
 v
existing LaunchPlan
```

### 6.2 Offline Account Flow

```text
OfflineAccountSpec
      |
      v
validate name
      |
      v
stable AccountId + stable profile UUID
      |
      v
persist public Account metadata
      |
      v
AccountService::launch_session(account_id)
      |
      v
existing LaunchSession
```

No secret store is required for a normal offline account.

### 6.3 Managed Java Flow

```text
JavaRequirement
      |
      v
local discovery/probe
      |
      +--> compatible local runtime -> JavaRuntime
      |
      v
managed runtime inventory
      |
      +--> compatible managed runtime -> JavaRuntime
      |
      v
JavaDistributionProvider
      |
      v
ManagedJavaRelease
      |
      v
ManagedJavaInstallPlan
      |
      v
Artifact acquisition
      |
      v
bounded safe extraction
      |
      v
probe staged java executable
      |
      v
transactional commit
      |
      v
ManagedJavaRuntime
      |
      v
JavaRuntime
      |
      v
existing LaunchPlan
```

---

## 7. Repository Transition

Phase 2 activates one new bounded-context crate and expands existing crates without creating a
second service layer.

Target high-level workspace:

```text
graphene/
├── src/
│   └── lib.rs
├── crates/
│   ├── graphene-core/
│   ├── graphene-platform/
│   ├── graphene-network/
│   ├── graphene-storage/
│   ├── graphene-minecraft/
│   ├── graphene-instance/
│   ├── graphene-auth/              # activated in Phase 2
│   ├── graphene-java/              # expanded with managed-runtime domain
│   ├── graphene-providers/         # Microsoft + Java distribution adapters
│   ├── graphene-install/
│   ├── graphene-launch/
│   └── graphene-service/           # wiring/orchestration only
├── tests/
│   ├── fixtures/
│   │   ├── auth/
│   │   └── java-managed/
│   ├── phase0_foundation.rs
│   ├── phase1_vanilla.rs
│   └── phase2_auth_java.rs
└── docs/
    ├── PHASE_2_IMPLEMENTATION_PLAN.md
    ├── PHASE_2_API.md              # produced with implementation
    ├── PHASE_2_SECURITY_REVIEW.md  # produced with implementation
    ├── PHASE_2_AUTH_SMOKE_TEST.md  # produced with implementation
    └── PHASE_2_JAVA_SMOKE_TEST.md  # produced with implementation
```

Phase 2 should not activate `graphene-content`, `graphene-pack`, or `graphene-diagnostics`.

---

## 8. Mandatory Thin-Crate-Root Rule

Phase 2 must preserve the modularization rule already added to `CONTRIBUTING.md`:

> Crate roots are public facades, not business-logic containers.

No Phase 2 crate may put the authentication flow, token parsing, account repository, keyring logic,
runtime archive extraction, provider HTTP logic, or managed-runtime installation logic directly in
`lib.rs`.

`lib.rs` should contain only:

- crate-level documentation;
- `mod` declarations;
- deliberate `pub use` exports;
- very small compile-time constants where genuinely crate-global.

A review should reject a Phase 2 change if the implementation is technically split into crates but
each crate hides a new monolithic `lib.rs`.

Architecture tooling should be expanded to enforce this structurally where practical.

---

## 9. Proposed Phase 2 Module Layout

### 9.1 `graphene-auth`

```text
crates/graphene-auth/src/
├── lib.rs
├── account/
│   ├── mod.rs
│   ├── id.rs
│   ├── kind.rs
│   ├── profile.rs
│   ├── state.rs
│   └── record.rs
├── provider/
│   ├── mod.rs
│   ├── capability.rs
│   ├── interaction.rs
│   ├── request.rs
│   ├── result.rs
│   └── session.rs
├── offline/
│   ├── mod.rs
│   ├── create.rs
│   └── session.rs
├── refresh/
│   ├── mod.rs
│   └── policy.rs
├── repository/
│   ├── mod.rs
│   └── trait.rs
├── secret/
│   ├── mod.rs
│   ├── key.rs
│   └── material.rs
└── error.rs
```

Provider DTOs do **not** belong here.

### 9.2 `graphene-providers`

```text
crates/graphene-providers/src/
├── lib.rs
├── mojang/
│   └── ...
├── microsoft/
│   ├── mod.rs
│   ├── config.rs
│   ├── dto/
│   │   ├── mod.rs
│   │   ├── device_code.rs
│   │   ├── oauth.rs
│   │   ├── xbox.rs
│   │   └── minecraft.rs
│   ├── normalize.rs
│   ├── policy.rs
│   ├── provider.rs
│   ├── refresh.rs
│   └── error.rs
└── java_distribution/
    ├── mod.rs
    ├── capability.rs
    └── adoptium/
        ├── mod.rs
        ├── config.rs
        ├── dto.rs
        ├── normalize.rs
        ├── provider.rs
        └── error.rs
```

If the implementation chooses a different reference Java distribution, only the concrete adapter
subtree changes. The `graphene-java` domain must not.

### 9.3 `graphene-java`

```text
crates/graphene-java/src/
├── lib.rs
├── discovery.rs
├── error.rs
├── model.rs
├── probe.rs
├── selection.rs
├── version.rs
├── managed/
│   ├── mod.rs
│   ├── id.rs
│   ├── request.rs
│   ├── release.rs
│   ├── runtime.rs
│   ├── plan.rs
│   ├── inventory.rs
│   └── provider.rs
└── diagnostic/
    ├── mod.rs
    ├── compatibility.rs
    └── codes.rs
```

### 9.4 `graphene-storage`

```text
crates/graphene-storage/src/
├── lib.rs
├── atomic.rs
├── cache.rs
├── layout.rs
├── root.rs
├── temp.rs
├── account/
│   ├── mod.rs
│   ├── schema.rs
│   └── repository.rs
├── secret/
│   ├── mod.rs
│   ├── trait.rs
│   ├── encrypted_file.rs
│   └── memory.rs
└── runtime/
    ├── mod.rs
    ├── schema.rs
    └── repository.rs
```

OS keyring details may be implemented through `graphene-platform` and wrapped/wired by
`graphene-storage`.

### 9.5 `graphene-platform`

```text
crates/graphene-platform/src/
├── lib.rs
├── arch.rs
├── filesystem.rs
├── os.rs
├── paths.rs
├── process.rs
└── secret_store/
    ├── mod.rs
    ├── capability.rs
    └── keyring.rs
```

Platform only owns low-level OS secret-storage capability. It must not know what a Microsoft refresh
token is.

### 9.6 `graphene-service`

```text
crates/graphene-service/src/
├── lib.rs
├── builder.rs
├── context.rs
├── adapters.rs
├── account_service/
│   ├── mod.rs
│   ├── login.rs
│   ├── refresh.rs
│   ├── offline.rs
│   ├── repository.rs
│   └── launch_session.rs
├── managed_java_service/
│   ├── mod.rs
│   ├── resolve.rs
│   ├── install.rs
│   └── inventory.rs
├── java_service.rs
├── launch_service.rs
└── ...
```

`graphene-service` coordinates interfaces. Provider protocol parsing, encryption primitives,
filesystem archive parsing, and Java compatibility algorithms remain outside it.

---

## 10. Phase 2 Crate Responsibilities

### 10.1 `graphene-core`

Phase 2 additions:

- `AccountId`;
- any additional stable secret-redaction helper required by multiple contexts;
- new structured `ErrorCode` / `ErrorKind` variants;
- generic operation/diagnostic values only if existing types are insufficient.

Must not gain:

- account records;
- OAuth terminology;
- Xbox/Minecraft authentication protocol;
- Java distribution DTOs;
- keyring implementation.

### 10.2 `graphene-auth`

Owns:

- account identity;
- account kind/state/profile;
- provider-neutral authentication contracts;
- interactive-auth value types;
- authentication session semantics;
- refresh semantics;
- offline account rules;
- secret record identity;
- repository/provider ports.

Must not own:

- Reqwest;
- Microsoft endpoints;
- Xbox/Minecraft JSON DTOs;
- OS keyring API calls;
- launch argument construction;
- instance persistence;
- UI browser windows.

### 10.3 `graphene-platform`

Phase 2 additions:

- low-level keyring/credential-vault capability detection;
- platform-specific secure-store adapter;
- permissions and path primitives required by encrypted fallback;
- executable/archive platform normalization only when genuinely OS-specific.

Must not know:

- account IDs beyond opaque labels passed by storage;
- refresh-token semantics;
- Minecraft authentication stages.

### 10.4 `graphene-storage`

Phase 2 additions:

- account metadata repository;
- managed-runtime descriptor repository;
- secret-store abstraction/wiring;
- encrypted fallback store;
- schema versions;
- atomic metadata replacement;
- account/runtime enumeration.

Must not decide:

- whether an account may authenticate;
- when a refresh token should be used;
- which Java release is compatible;
- which provider to contact.

### 10.5 `graphene-network`

No authentication-domain logic is added.

It may gain narrowly justified generic primitives such as:

- form-encoded request support;
- bounded JSON POST helpers;
- explicit no-store response handling;
- cancellation-aware polling support only if truly generic.

It must not expose:

```text
post_microsoft_token(...)
post_xsts(...)
download_adoptium(...)
```

### 10.6 `graphene-java`

Owns:

- managed runtime identities;
- release descriptors;
- provider-neutral runtime requests;
- Java distribution provider interface;
- managed runtime inventory model;
- installation-plan domain model;
- compatibility diagnostics;
- deterministic selection policy.

Must not own:

- Adoptium DTOs;
- network transport;
- cache implementation;
- filesystem commit implementation;
- UI prompts.

### 10.7 `graphene-providers`

Phase 2 owns concrete adapters for:

- Microsoft identity protocol;
- Xbox/Minecraft authentication chain required for Microsoft accounts;
- the chosen reference Java distribution.

Provider DTOs remain private.

### 10.8 `graphene-launch`

Phase 2 should require little or no business-logic change.

It continues to own:

- `LaunchSession`;
- `LaunchRequest`;
- launch planning;
- process execution.

It must not depend on `graphene-auth`.

The service layer converts authenticated account state into `LaunchSession`.

### 10.9 `graphene-service`

Owns application orchestration:

- account operations;
- secret-store wiring;
- provider construction;
- refresh workflow orchestration;
- managed Java orchestration;
- conversion from `AuthSession` to `LaunchSession`;
- coordination with existing Java/Launch services.

It must not duplicate provider/domain algorithms.

### 10.10 Root `graphene`

The root crate only re-exports stable Phase 2 facade types and services.

Expected new facade direction:

```rust
graphene.accounts()
graphene.java()
graphene.launch()
```

No provider DTO or secret-store implementation type should be required by normal consumers unless
the caller explicitly configures an advanced adapter.

---

## 11. Required Dependency Direction

Conceptual Phase 2 graph:

```text
                              graphene
                                 |
                                 v
                         graphene-service
                     /       /    |    \       \
                    v       v     v     v       v
                 auth     java  launch storage providers
                  |        |       |      |       |
                  |        |       |      |       +--> network
                  |        |       |      |       +--> auth
                  |        |       |      |       +--> java
                  |        |       |      |
                  |        |       |      +--> platform
                  |        |       |
                  +--------+-------+------------------> core
```

Required edges:

```text
graphene-auth
    -> graphene-core

graphene-java
    -> graphene-core
    -> graphene-platform

graphene-storage
    -> graphene-core
    -> graphene-platform

graphene-providers
    -> graphene-core
    -> graphene-network
    -> graphene-auth
    -> graphene-java
    -> graphene-minecraft

graphene-service
    -> all activated domain/adapter crates

graphene-launch
    -> graphene-core
    -> graphene-java
    -> graphene-instance
    -> graphene-minecraft
    -> graphene-platform
```

Forbidden new edges:

```text
graphene-auth -> graphene-launch
graphene-auth -> graphene-service
graphene-auth -> graphene-network
graphene-auth -> graphene-storage
graphene-java -> graphene-providers
graphene-java -> graphene-network
graphene-java -> graphene-storage
graphene-launch -> graphene-auth
graphene-launch -> graphene-providers
graphene-storage -> graphene-auth
graphene-platform -> graphene-auth
graphene-service -> Microsoft DTO modules
graphene-service -> Adoptium DTO modules
any backend crate -> tauri
any backend crate -> slint
```

Interfaces should invert dependencies when orchestration needs a capability owned by a lower layer.

---

## 12. Account Domain Model

The account model must distinguish stable identity, public profile data, provider state, and
ephemeral credentials.

Conceptual model:

```rust
pub struct Account {
    pub id: AccountId,
    pub kind: AccountKind,
    pub profile: AccountProfile,
    pub state: AccountState,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
```

`Timestamp` may use an existing/narrow dependency chosen during implementation; public serialization
must remain stable and timezone-unambiguous.

### 12.1 `AccountKind`

Phase 2 variants:

```text
Microsoft
Offline
```

The enum should be non-exhaustive for later Yggdrasil/authlib-injector support.

### 12.2 `AccountProfile`

Provider-neutral public data should include only fields useful to hosts and launching:

```text
display_name
minecraft_profile_id / UUID
optional skin metadata summary
optional cape metadata summary
```

Phase 2 does not require downloading or mutating skin/cape content.

### 12.3 `AccountState`

Recommended normalized states:

```text
Ready
RequiresReauthentication
Unavailable
```

Offline accounts normally remain `Ready`.

Microsoft accounts may become `RequiresReauthentication` after refresh rejection or credential
removal.

Provider-specific raw error values must not become account-state variants.

---

## 13. Account Identity

`AccountId` must be a Graphene-owned opaque typed identifier added beside existing typed IDs.

The account ID must not be:

- the Microsoft user ID;
- the XUID;
- the Minecraft UUID;
- the refresh token;
- an email address;
- the display name.

Those values may change or may be provider-specific.

`AccountId` is the stable local Graphene identity used for:

- repository paths;
- secret-store keys;
- service APIs;
- future per-instance account references.

---

## 14. Public Account Persistence

Recommended layout:

```text
graphene-data/
└── config/
    └── accounts/
        ├── <account-id>.json
        └── ...
```

Account JSON contains only public/non-secret metadata.

Conceptual schema:

```json
{
  "schema_version": 1,
  "account_id": "...",
  "kind": "microsoft",
  "profile": {
    "display_name": "...",
    "minecraft_uuid": "..."
  },
  "state": "ready",
  "created_at": "...",
  "updated_at": "..."
}
```

The exact schema is implementation work, but it must not contain:

- Microsoft access token;
- Microsoft refresh token;
- Xbox user token;
- XSTS token;
- Minecraft services access token;
- device code;
- user code;
- credential-bearing URLs;
- secret-store encryption key.

Account writes must use atomic replacement.

---

## 15. Secret Classification

Phase 2 must explicitly classify authentication values.

### 15.1 Persistent Secret

The normal persistent Microsoft credential is:

```text
Microsoft refresh credential
```

If the provider rotates the refresh credential, the stored value must be replaced after successful
refresh.

### 15.2 Ephemeral Secrets

The following remain in memory and must not be persisted:

```text
Microsoft access token
device code
Xbox user token
XSTS token
Minecraft access token
temporary proof/verifier values
```

A user-facing device **user code** is sensitive during the interaction lifetime even if it is not a
long-lived credential. It must not be logged or persisted.

### 15.3 Public Values

Values such as these may be persisted if required:

```text
AccountId
Minecraft profile UUID
Minecraft profile name
account kind
last successful authentication timestamp
non-secret provider configuration identity
```

Emails should not be persisted unless a concrete product requirement exists.

---

## 16. Secret Store Architecture

The secret store is a capability, not a Microsoft-specific repository.

Conceptual interface:

```rust
pub trait SecretStore: Send + Sync {
    fn get(...);
    fn put(...);
    fn delete(...);
}
```

The actual async/sync shape is implementation-specific but must avoid blocking async workers with
platform credential APIs.

### 16.1 Secret Record Key

Use a Graphene-owned opaque key:

```text
namespace: graphene
record kind: account-auth
record id: AccountId
```

Do not use display name or email as the primary secret key.

### 16.2 Default Production Policy

Preferred order:

```text
OS secure keyring / credential vault
        |
        +--> explicit encrypted-file fallback when configured
        |
        +--> otherwise structured SecretStoreUnavailable failure
```

Graphene must never silently downgrade to plaintext.

### 16.3 Keyring Backend

Platform code owns the OS-specific secure storage adapter.

The storage layer owns mapping between Graphene records and the platform secret-store capability.

The auth layer only sees the abstract secret repository.

### 16.4 Encrypted-File Fallback

An encrypted fallback may be supported only if:

- encryption is authenticated;
- a fresh nonce is used per write;
- the encryption key is not stored unprotected next to the ciphertext;
- an unlock secret/key is supplied by the host or derived from an explicitly supplied passphrase;
- passphrase derivation uses a memory-hard KDF;
- file permissions are restricted where the platform supports it;
- atomic replacement is used;
- format/schema version is explicit;
- corruption/authentication failure is a structured error.

A fallback encryption key embedded in the binary or persisted in the same directory is explicitly
forbidden.

### 16.5 Test Secret Store

A deterministic in-memory store may exist for tests.

It must require explicit fixture/test construction and must not be the production default.

---

## 17. Microsoft Application Configuration

Graphene must not ship a borrowed, undocumented, or third-party Microsoft OAuth client ID.

A `MicrosoftAuthConfig` is supplied explicitly or through a distributor-owned configuration layer.

Conceptual fields:

```text
client_id
tenant policy
requested scopes
device authorization endpoint
token endpoint
Xbox/Minecraft service endpoints
HTTP policy
fixture mode
```

Production configuration must require secure HTTPS endpoints.

Fixture configuration may allow local HTTP only through an explicit test constructor matching the
existing Phase 1 provider fixture philosophy.

The client ID is not a secret, but it identifies the application registration and must be treated as
configuration rather than hard-coded protocol magic.

---

## 18. Required Microsoft Interactive Flow

Phase 2 requires a device-authorization flow as the baseline UI-independent Microsoft sign-in path.

The design must not require Graphene to own:

- a browser window;
- an embedded webview;
- a localhost UI page;
- a Tauri callback;
- a Slint callback.

The provider creates a typed interaction value that the host can present.

Conceptual interaction:

```rust
pub enum AuthInteraction {
    DeviceCode {
        verification_uri: String,
        user_code: SensitiveString,
        expires_at: Timestamp,
        poll_interval: Duration,
        message: Option<String>,
    },
}
```

The exact public representation may avoid `SensitiveString` for host convenience, but any type that
reveals the user code must have intentionally redacted formatting and no persistence support.

---

## 19. Interaction/Operation Boundary

A Microsoft login is a long-running operation.

Recommended host flow:

```text
let login = graphene.accounts().begin_microsoft_login(...).await?;
let interaction = login.interaction().await?;
host presents interaction
let account = login.await_result().await?;
```

Alternative event-based shapes are acceptable if they preserve the same guarantees.

The operation must:

- expose an `OperationId`;
- be cancellable;
- emit bounded progress/stage events;
- surface the interactive challenge exactly as typed data;
- never log the user code;
- terminate on expiration;
- obey server-requested polling intervals;
- handle slow-down responses;
- avoid busy polling.

Suggested stages:

```text
request_device_code
await_user_authorization
exchange_microsoft_token
authenticate_xbox
authorize_xsts
authenticate_minecraft
verify_entitlement
fetch_profile
persist_account
persist_refresh_credential
complete
```

---

## 20. Microsoft Authentication Provider Stages

The concrete provider adapter owns the protocol chain.

The normalized logical stages are:

```text
Microsoft identity authorization
        |
        v
Microsoft access/refresh result
        |
        v
Xbox user authentication
        |
        v
XSTS authorization
        |
        v
Minecraft services authentication
        |
        +--> entitlement
        |
        +--> Minecraft profile
        |
        v
Normalized Microsoft AuthSession
```

The domain/service layers must not know the raw HTTP DTO format of any stage.

### 20.1 Protocol Volatility Rule

Authentication endpoints, scopes, error payloads, and service requirements are externally controlled
and may change.

During Phase 2 implementation:

- verify production endpoint/scopes against current official Microsoft documentation;
- isolate all such values under provider configuration/policy modules;
- freeze fixture responses;
- do not spread literal URLs through the codebase;
- do not make provider response fields part of public APIs.

---

## 21. Entitlement and Profile Validation

A Microsoft account should not be persisted as `Ready` until Graphene has enough evidence that the
account can represent a Minecraft Java profile for launch.

Phase 2 activation should require:

1. Minecraft services authentication succeeds.
2. Entitlement/ownership validation succeeds according to the provider's current supported contract.
3. Minecraft profile retrieval succeeds.
4. The normalized profile contains a usable profile ID and name.

Failure cases must be structured and distinguish at least:

```text
authentication rejected
Xbox authentication unavailable
XSTS authorization rejected
Minecraft service authentication rejected
entitlement missing
Minecraft profile missing
provider response invalid
```

Graphene must not infer ownership solely because Microsoft login succeeded.

---

## 22. Authentication Session Model

`graphene-auth` should produce a provider-neutral `AuthSession` that contains the values required to
construct a launch session plus lifecycle metadata.

Conceptual shape:

```text
AuthSession
├── AccountId
├── public profile
├── access credential (ephemeral)
├── optional client/xuid metadata required by launch placeholders
├── issued/expiry metadata
└── account kind
```

`AuthSession` must:

- redact secrets;
- avoid serialization for secret-bearing fields;
- be short-lived;
- not become the persistent account record.

---

## 23. Account-to-LaunchSession Conversion

The existing Phase 1 `LaunchSession` remains the launch boundary.

Recommended ownership:

```text
graphene-auth -> AuthSession
graphene-service -> conversion
graphene-launch -> LaunchSession
```

Do **not** add:

```text
graphene-launch -> graphene-auth
```

The conversion must map both provider types into the same Phase 1 structure.

### 23.1 Microsoft

A ready Microsoft session maps normalized profile/credential values into the existing
`LaunchSession`.

### 23.2 Offline

An offline account creates a deterministic ephemeral launch session from public account metadata.

No offline credential is persisted.

### 23.3 Launch Planning Invariant

After a `LaunchSession` is produced:

```text
Minecraft launch argument logic is identical
```

Phase 2 must not add:

```text
if microsoft ...
if offline ...
```

inside placeholder construction or process execution.

---

## 24. Microsoft Session Refresh

`graphene.accounts().launch_session(account_id)` should be able to refresh a Microsoft account
without host-side token logic.

Recommended flow:

```text
load account metadata
      |
      v
load refresh credential from SecretStore
      |
      v
refresh Microsoft identity token
      |
      v
repeat Xbox/XSTS/Minecraft service chain
      |
      v
validate profile/entitlement
      |
      +--> rotate stored refresh credential if returned
      |
      v
return ephemeral AuthSession / LaunchSession
```

### 24.1 Refresh Failure Classes

Distinguish:

```text
temporary network failure
provider service failure
credential rejected/revoked
interaction required
account no longer entitled
profile unavailable
secret store unavailable
```

A temporary network failure must not mark the account permanently invalid.

A confirmed refresh credential rejection should update public state to
`RequiresReauthentication`.

### 24.2 Refresh Rotation

When a successful response provides a replacement refresh credential:

1. keep the old credential in memory until the new authentication chain is proven usable;
2. persist the new credential atomically through the secret store;
3. only then discard the old in-memory value;
4. never write both to logs/errors.

Implementation may adjust exact ordering to the backend's atomicity guarantees, but it must avoid
destroying the only usable credential before the replacement is durable.

---

## 25. In-Memory Session Cache

Phase 2 may maintain a bounded in-engine cache of ephemeral authenticated sessions.

Requirements:

- cache lifetime is bounded;
- cache is per `Graphene` engine, not global;
- cache entries are keyed by `AccountId`;
- secrets remain in memory only;
- cache is invalidated on account removal;
- cache does not change correctness;
- cache misses fall back to refresh;
- no session is serialized.

This is an optimization, not the source of truth.

---

## 26. Offline Account Model

Offline accounts are local identities, not fake Microsoft accounts.

Recommended input:

```rust
pub struct OfflineAccountSpec {
    pub display_name: String,
    pub profile_uuid: Option<Uuid>,
}
```

Rules:

- validate non-empty bounded username;
- reject NUL/control/path-dangerous content;
- normalize surrounding whitespace according to a documented policy;
- persist a stable profile UUID;
- if no UUID is supplied, generate one once and persist it;
- do not regenerate UUID per launch;
- do not create secret-store records;
- make provider kind explicitly `Offline`.

Graphene should not claim that an offline account provides authenticated access to online-mode
servers.

---

## 27. Account Repository API

Conceptual service behavior:

```text
accounts().list()
accounts().get(account_id)
accounts().create_offline(spec)
accounts().begin_microsoft_login(...)
accounts().reauthenticate(account_id)
accounts().launch_session(account_id)
accounts().remove(account_id)
```

Optional Phase 2 behavior:

```text
accounts().rename_offline(account_id, new_name)
```

Microsoft profile names should normally be refreshed from the provider rather than arbitrarily
renamed locally.

### 27.1 Removal Semantics

Removing a Microsoft account must attempt to:

1. remove secret-store record;
2. remove public account metadata;
3. invalidate in-memory session cache.

The implementation must define recoverable behavior if secret deletion succeeds but metadata
deletion fails or vice versa.

No removal API may silently leave a known persistent refresh credential behind.

---

## 28. Account Concurrency

Concurrent operations on one account require per-account coordination.

At minimum:

```text
refresh(account A) ----+
                       +--> one credential-refresh critical section
refresh(account A) ----+
```

Different accounts should remain independently concurrent.

Required behavior:

- duplicate refreshes do not race refresh-token rotation;
- removal cannot race a refresh into recreating secrets unexpectedly;
- reauthentication and refresh have explicit ordering;
- cancellation does not leave half-written metadata.

A keyed async gate similar in spirit to the Phase 0 artifact gate may be used at service level.

---

## 29. Authentication Error Contract

Add stable machine-readable families.

Suggested codes:

```text
AUTH_REQUEST_INVALID
AUTH_PROVIDER_UNAVAILABLE
AUTH_INTERACTION_EXPIRED
AUTH_INTERACTION_REQUIRED
AUTH_DEVICE_CODE_REJECTED
AUTH_REFRESH_REJECTED
AUTH_XBOX_REJECTED
AUTH_XSTS_REJECTED
AUTH_MINECRAFT_REJECTED
AUTH_ENTITLEMENT_MISSING
AUTH_PROFILE_MISSING
AUTH_ACCOUNT_NOT_FOUND
AUTH_ACCOUNT_STATE_INVALID
AUTH_SECRET_UNAVAILABLE
AUTH_SECRET_READ_FAILED
AUTH_SECRET_WRITE_FAILED
AUTH_SECRET_DELETE_FAILED
AUTH_PERSISTENCE_FAILED
AUTH_CANCELLED
```

Exact enum names should follow existing `ErrorCode` style and remain stable after publication.

Add `ErrorKind::Auth` and, if useful, `ErrorKind::SecretStorage`.

Provider status/error codes may appear only as carefully screened non-secret diagnostic context.

---

## 30. Authentication Diagnostics

Phase 2 should expose structured diagnostic evidence for conditions that are useful to hosts without
parsing errors.

Examples:

```text
AUTH_REAUTHENTICATION_REQUIRED
AUTH_SECRET_STORE_UNAVAILABLE
AUTH_ENTITLEMENT_MISSING
AUTH_PROVIDER_TEMPORARILY_UNAVAILABLE
```

Diagnostics are data, not localized prose.

The host decides presentation.

---

## 31. Authentication Security Requirements

Phase 2 must treat authentication as a high-risk boundary.

Required controls:

- production endpoints require HTTPS;
- TLS validation remains enabled;
- response body sizes are bounded;
- redirects follow explicit policy;
- no credential-bearing URLs in error context;
- device/user codes are redacted;
- access/refresh/Xbox/XSTS/Minecraft tokens are redacted;
- no token may derive a filename;
- no token may enter tracing fields;
- no full auth response body may be attached to an error;
- refresh credentials are stored only through `SecretStore`;
- access credentials are memory-only;
- provider DTOs containing secrets implement custom redacted `Debug` or no `Debug`;
- secret-bearing types do not derive `Serialize`;
- cancellation clears temporary secret state as promptly as practical;
- test fixtures use unmistakably fake credentials;
- exported diagnostics run through existing/expanded redaction checks.

---

## 32. Microsoft Flow Choice and Future PKCE

Phase 2 requires device authorization because it preserves UI independence cleanly.

Authorization-code + PKCE may be added later behind the same provider-neutral interaction
abstraction.

The Phase 2 domain should therefore not define:

```text
MicrosoftDeviceCodeAccount
```

as a permanent account kind.

The interaction is a login mechanism, not the account identity.

Future interaction variants may include:

```text
OpenBrowser
LoopbackRedirect
AuthorizationCodePkce
```

without changing persistent account records or launch behavior.

---

## 33. Managed Java Objective

Phase 1 already provides:

- candidate discovery;
- bounded process probing;
- version/vendor/architecture normalization;
- compatibility checking;
- deterministic local selection.

Phase 2 extends Java from:

```text
find compatible installed Java or fail
```

to:

```text
find compatible local Java
    |
    +--> if available according to policy, use it
    |
    v
find compatible managed Java
    |
    +--> if available, use it
    |
    v
resolve/install compatible managed Java
    |
    v
use same JavaRuntime contract
```

The existing `JavaRuntime` remains the launch-facing runtime value.

---

## 34. Java Distribution Provider Interface

`graphene-java` owns a provider-neutral distribution port.

Conceptual behavior:

```text
resolve_release(requirement, platform, policy)
        |
        v
ManagedJavaRelease
```

The provider interface should expose capabilities such as:

```text
release lookup
archive download
checksum metadata
supported OS
supported architecture
image type
JVM implementation
```

The domain must not expose provider DTOs.

---

## 35. Reference Managed-Java Provider

Phase 2 should implement one concrete reference provider to prove the abstraction.

Recommended reference direction:

```text
Eclipse Temurin / Adoptium API
```

Reasons:

- cross-platform archive distribution;
- machine-readable release metadata;
- checksum metadata;
- no requirement to execute a privileged installer;
- straightforward mapping to Graphene `Artifact`.

This is a Phase 2 implementation choice, not a permanent lock-in.

The provider must remain replaceable so later work can add:

- Microsoft Build of OpenJDK;
- Mojang/Minecraft runtime distributions where a stable supported contract is available;
- Azul;
- Amazon Corretto;
- other distributions.

The reference provider must be hidden behind `JavaDistributionProvider`.

---

## 36. Managed Java Request

Conceptual model:

```rust
pub struct ManagedJavaRequest {
    pub requirement: JavaRequirement,
    pub platform: JavaPlatform,
    pub image: JavaImageKind,
    pub vendor_policy: JavaVendorPolicy,
}
```

Phase 2 should normally request:

```text
GA release
matching required Java major
matching current OS
matching current architecture
JRE or JDK according to documented policy
HotSpot-compatible runtime
normal heap build
```

The exact provider query belongs in the adapter.

### 36.1 Image Policy

For Minecraft launch, a JRE is sufficient in principle when the distribution provides the required
runtime components.

However, provider availability differs.

The domain should model:

```text
Jre
Jdk
```

and allow a deterministic fallback policy.

Do not assume every provider/platform/major offers both.

---

## 37. Managed Java Release Model

Normalized release data should include:

```text
provider identity
distribution
Java major
full version
vendor
OS
architecture
image kind
JVM implementation
archive artifact
archive format
release identity
```

The archive `Artifact` should contain trustworthy SHA-256 where the provider supplies it.

No managed runtime should be installed from an unverifiable remote archive under normal production
policy.

---

## 38. Managed Runtime Identity

Managed runtimes require a stable Graphene-owned identity separate from the executable path.

Conceptual identity fields:

```text
distribution
major
full version
OS
architecture
image
provider release ID
```

A `ManagedRuntimeId` may be added as a typed Graphene ID or deterministic domain identifier.

The persistent descriptor must not identify a runtime only by "Java 21", because multiple builds may
coexist during update/rollback.

---

## 39. Managed Runtime Storage Layout

Recommended layout:

```text
graphene-data/
└── shared/
    └── runtimes/
        └── <managed-runtime-id>/
            ├── runtime.json
            └── runtime/
                ├── bin/
                └── ...
```

Staging:

```text
shared/runtimes/.staging-<operation-id>/
```

A committed runtime directory must contain a schema-versioned `runtime.json`.

Descriptor contents may include:

```text
schema_version
managed_runtime_id
provider
distribution
version
major
vendor
architecture
image_kind
java_relative_path
archive digest
installed_at
```

Persist relative paths, not absolute data-root paths.

---

## 40. Managed Java Install Plan

Managed Java must follow the project's "plan before execute" rule.

Conceptual:

```rust
pub struct ManagedJavaInstallPlan {
    pub plan_version: u32,
    pub runtime: PlannedManagedRuntime,
    pub archive: Artifact,
    pub staging_actions: Vec<...>,
}
```

The plan is generated before committed runtime mutation.

Plan validation must reject:

- unsupported platform/architecture;
- missing verifiable integrity;
- unsafe destination;
- conflicting managed runtime identity;
- unsupported archive type;
- impossible Java executable relative path.

---

## 41. Reuse Existing Artifact Pipeline

Managed Java must not create a second downloader.

Required flow:

```text
ManagedJavaRelease.archive
        |
        v
Graphene Artifact
        |
        v
existing ArtifactAcquirer
        |
        v
verified shared cache object
        |
        v
runtime staging extraction
```

This preserves:

- retry policy;
- redirects;
- proxy behavior;
- cancellation;
- integrity verification;
- cache reuse;
- bounded network concurrency;
- duplicate download gates.

The Java provider resolves metadata. It does not download files itself when the common artifact
pipeline can do so.

---

## 42. Managed Runtime Archive Security

Managed Java archives are untrusted remote input.

Phase 2 must safely support only the archive formats required by the chosen provider/platform set.

Likely baseline:

```text
ZIP
tar.gz
```

Required protections:

- no absolute paths;
- no `..` traversal;
- no Windows prefix escape;
- no backslash ambiguity;
- no unsafe symbolic/hard links;
- bounded archive file size;
- bounded entry count;
- bounded per-entry expanded size;
- bounded total expanded size;
- decompression ratio limit;
- reject unsupported special file types;
- preserve executable permission only where required and safe;
- never execute provider-supplied post-install scripts.

Archive extraction should reuse safe archive primitives if the existing Phase 1 implementation can
be generalized cleanly. It must not copy-paste a second vulnerable extractor.

---

## 43. Runtime Staging and Commit

Required sequence:

1. validate `ManagedJavaInstallPlan`;
2. acquire per-runtime install lock/gate;
3. acquire and verify archive through common artifact pipeline;
4. create managed staging directory;
5. extract archive safely;
6. locate expected Java executable;
7. verify managed path containment;
8. probe staged Java executable;
9. verify major/architecture compatibility;
10. write `runtime.json` into staging;
11. revalidate staged runtime;
12. seal cancellation immediately before non-interruptible publication;
13. publish committed runtime directory safely;
14. clean staging;
15. return `ManagedJavaRuntime` / `JavaRuntime`.

Failure before publication must not create a valid-looking committed runtime.

---

## 44. Runtime Probe Before Commit

A downloaded runtime is not trusted merely because the archive hash matches.

The staged `java` executable must be probed using the existing Phase 1 bounded probe path.

The probe must verify at minimum:

```text
process can execute
major version matches requirement
architecture is compatible
vendor/version parse is coherent
```

Only then can the runtime be committed.

This ensures the managed runtime contract converges on the same `JavaRuntime` used by local Java.

---

## 45. Managed Java Inventory

`graphene-java`/storage should support deterministic inventory of committed managed runtimes.

Conceptual service behavior:

```text
java().managed().list()
java().managed().install(requirement)
java().managed().remove(runtime_id)     # optional Phase 2 if safe
```

At minimum, Phase 2 requires list/read/install and selection.

Removal may be deferred if reference tracking is not yet safe before Phase 4.

No managed runtime should be considered valid solely because its directory exists; its descriptor
and executable must validate.

---

## 46. Java Selection Policy After Phase 2

Existing Phase 1 selection needs an explicit extended policy.

Recommended default:

```text
1. explicit Java override from caller
2. compatible already-discovered local Java
3. compatible committed Graphene-managed Java
4. install managed Java if caller/policy allows
5. structured JavaIncompatible / JavaNotFound result
```

Alternative preference policies may be exposed later, but the default must be deterministic.

A managed runtime must never unexpectedly override an explicit host path.

### 46.1 Auto-Install Policy

Automatic managed Java download is a side effect and must be explicit.

Recommended API separation:

```text
java().select_for_instance(...)         # no download side effect
java().ensure_for_instance(...)         # may install managed runtime
```

This keeps dry-run/inspection behavior predictable.

`launch().plan()` should not suddenly perform a large Java download unless the API explicitly
documents and opts into that behavior.

---

## 47. Java Compatibility Diagnostics

Phase 2 must add structured diagnostic production around the existing Java requirement model.

Examples:

```text
JAVA_NOT_FOUND
JAVA_VERSION_TOO_OLD
JAVA_VERSION_TOO_NEW            # if future requirement range needs it
JAVA_ARCHITECTURE_MISMATCH
JAVA_RUNTIME_UNEXECUTABLE
JAVA_MANAGED_PROVIDER_UNAVAILABLE
JAVA_MANAGED_RELEASE_UNAVAILABLE
JAVA_MANAGED_RUNTIME_CORRUPT
```

Diagnostic parameters may include safe values such as:

```text
required_major
found_major
required_arch
found_arch
vendor
runtime_source
```

Do not include arbitrary executable environment dumps.

---

## 48. Java Requirement Evolution

Current Phase 1 `JavaRequirement` stores:

```text
major_version
component_hint
```

Phase 2 should preserve compatibility.

If implementation evidence requires richer constraints, extend rather than replace it.

Potential future shape:

```text
exact/preferred major
minimum/maximum bounds
architecture requirements
component/distribution hint
```

Do not introduce complexity without a real Phase 2 provider/fixture requirement.

---

## 49. Managed Java Provider Configuration

Concrete provider configuration belongs in `graphene-providers`.

Conceptual:

```rust
pub struct ManagedJavaProviderConfig {
    pub metadata_base:...,
    pub allow_http: bool,   // fixture-only
    pub ...
}
```

Production defaults:

- HTTPS only;
- bounded metadata bodies;
- safe redirects;
- no credentials in URLs;
- normal shared network config.

Fixture mode:

- explicit local origin;
- frozen DTOs;
- deterministic release responses;
- deterministic archives;
- fake Java executable fixtures where platform permits.

---

## 50. Data-Root Layout Changes

Phase 2 requires at least:

```text
config/accounts/
shared/runtimes/
```

`shared/runtimes/` already exists in the Phase 0 layout.

`config/accounts/` may be added additively.

Changing the data-root layout marker version is required only if the implementation changes an
invariant whose interpretation differs for an existing root. Merely creating an additional managed
subdirectory may remain backward-compatible.

Any version bump must include migration behavior and tests.

---

## 51. Account Schema Versioning

Define:

```text
ACCOUNT_SCHEMA_VERSION = 1
```

The schema must be migration-friendly.

Deserialization must:

- reject unsupported future required versions;
- validate account ID consistency;
- validate account kind/state combinations;
- reject secret fields if legacy/malformed data attempts to include them;
- bound string lengths;
- reject path-dangerous values where relevant.

Provider-specific response JSON is never persisted as the account schema.

---

## 52. Managed Runtime Schema Versioning

Define:

```text
MANAGED_RUNTIME_SCHEMA_VERSION = 1
```

The descriptor must be provider-neutral enough that a future provider does not require a new global
runtime model.

It may retain provider identity and provider release ID as opaque metadata.

A runtime descriptor is not a substitute for probing; inventory validation should be able to mark a
runtime invalid/corrupt.

---

## 53. Builder and Composition Changes

`GrapheneBuilder` should gain explicit Phase 2 configuration without becoming provider-heavy.

Conceptual direction:

```text
GrapheneBuilder
├── network(...)
├── minecraft_provider(...)
├── microsoft_auth(...)
├── secret_store_policy(...)
├── managed_java_provider(...)
└── ...
```

Advanced provider-specific types may be accepted through builder methods and re-exported from the
facade, but the built `Graphene` object should expose normalized services.

`ServiceContext` may gain:

```text
account repository
secret store
auth provider registry / Microsoft provider
managed Java provider
account keyed gates
runtime keyed gates
ephemeral auth-session cache
```

Each field should be an interface/adapter object where ownership requires inversion.

---

## 54. Facade Expansion

Expected root facade direction:

```rust
let accounts = graphene.accounts();
let java = graphene.java();
let launch = graphene.launch();
```

Potential account flow:

```rust
let login = graphene.accounts().begin_microsoft_login().await?;
let interaction = login.interaction().await?;
let account = login.await_result().await?;
let session = graphene.accounts().launch_session(account.id).await?;
```

Potential offline flow:

```rust
let account = graphene.accounts().create_offline(spec).await?;
let session = graphene.accounts().launch_session(account.id).await?;
```

Potential managed Java flow:

```rust
let runtime = graphene
.java()
.ensure_for_instance(instance_id, policy)
.await?;
```

These examples are directional, not fixed signatures.

---

## 55. Launch-Service Integration

Phase 2 should add a convenience path without breaking existing Phase 1 explicit-session APIs.

Possible service-level addition:

```text
launch().plan_for_account(instance_id, account_id, ...)
```

Implementation flow:

```text
AccountService::launch_session(account_id)
        |
        v
LaunchRequest { session, ... }
        |
        v
existing LaunchService::plan(...)
```

The existing `LaunchRequest` path should remain available for advanced/ephemeral callers.

Do not move account persistence or refresh into `graphene-launch`.

---

## 56. Java-Service Integration

Existing:

```text
JavaService::select_for_instance(instance_id, explicit)
```

Phase 2 should distinguish:

```text
select_for_instance      -> local/committed selection only
ensure_for_instance      -> may install managed runtime
managed inventory        -> inspect committed managed runtimes
install_managed          -> explicit installation
diagnose_for_instance    -> structured compatibility evidence
```

Exact names may change, but side effects must be obvious from API naming/documentation.

---

## 57. Progress Model — Authentication

Authentication operations should use stage-based progress rather than fake percentages.

Example:

```text
Microsoft Login
├── Request device code             done
├── Await user authorization        active / indeterminate
├── Microsoft token                 done
├── Xbox authentication             done
├── XSTS                            done
├── Minecraft login                 done
├── Entitlement                     done
├── Profile                         done
└── Persist                         done
```

Do not expose token contents in progress events.

---

## 58. Progress Model — Managed Java

Managed Java can use nested progress:

```text
Ensure Java 21
├── Inspect local runtimes          done
├── Inspect managed runtimes        done
├── Resolve managed release         done
├── Download archive                63%
├── Verify archive                  done
├── Extract runtime                 41%
├── Probe runtime                   waiting
└── Commit                          waiting
```

Download bytes should come from the existing artifact acquisition operation where possible.

---

## 59. Cancellation — Authentication

Cancellation checkpoints must exist:

- before device-code request;
- while waiting for user authorization;
- between polling attempts;
- before each downstream provider stage;
- before account/secret persistence.

Once a credential write enters a non-interruptible atomic backend operation, cancellation may be
sealed for that small critical section.

Cancellation must not:

- persist a half-created Microsoft account;
- delete an existing valid account during reauthentication;
- leave a rotated refresh credential only in temporary plaintext storage.

---

## 60. Cancellation — Managed Java

Cancellation checkpoints:

- release resolution;
- artifact acquisition;
- archive extraction;
- staged validation;
- runtime probe;
- immediately before commit.

The same point-of-no-return philosophy from Phase 0/1 applies.

Verified cache objects acquired before cancellation may remain reusable.

Staging directories must be cleaned.

---

## 61. Blocking Work Policy

Potentially blocking Phase 2 work includes:

- keyring API calls;
- KDF/encryption operations;
- account JSON filesystem writes;
- archive hashing/extraction;
- managed-runtime filesystem validation;
- process probing.

Blocking operations must not stall async I/O worker threads.

Use explicit blocking boundaries where required.

No cancellation seal should occur before an unbounded blocking lock wait.

---

## 62. Network Safety

Authentication and managed Java metadata use the common network policy.

Required:

- bounded connect/request timeouts;
- bounded response bodies;
- explicit redirect policy;
- proxy support from engine network config;
- TLS validation;
- no HTTPS-to-HTTP downgrade;
- cancellation;
- safe retry classification.

### 62.1 Authentication Retry Rule

Do not blindly retry credential-bearing POST requests.

Each provider operation must classify whether retry is safe.

Device-code polling follows protocol-specific interval semantics.

Refresh/token exchange retries must avoid creating uncontrolled duplicate authorization behavior.

### 62.2 Managed Java Retry Rule

Read-only metadata requests and immutable archive downloads may reuse bounded generic retry
behavior.

---

## 63. URL and Error Redaction

Authentication errors must never include full request URLs if query strings or fragments could
contain sensitive material.

Provider errors should attach only safe context such as:

```text
stage = "xsts"
status = "401"
account_id = "..."
```

Do not attach:

```text
Authorization headers
request body
response body
access token
refresh token
device code
user code
XSTS token
Minecraft token
```

---

## 64. Secret-Type Hardening

Existing `SensitiveString` is a useful Phase 1 foundation.

Phase 2 should review whether it needs:

- explicit zeroization on drop;
- non-clone or carefully controlled clone semantics for selected credential types;
- secret byte support;
- intentional reveal scopes;
- constant-time equality only where relevant.

Do not add security dependencies mechanically. Any change should have a clear threat-model benefit
and be recorded.

At minimum, secret types must remain:

- non-serializable;
- redacted in `Debug`;
- redacted in `Display`;
- absent from `ErrorContext`.

---

## 65. Encrypted Fallback Threat Model

If encrypted fallback is implemented, document:

**Protects against:**

- casual plaintext discovery;
- accidental credential leakage from copying the Graphene data root;
- offline inspection without the unlock secret.

**Does not protect against:**

- malware running as the same user while Graphene is unlocked;
- a compromised host process;
- a stolen passphrase;
- debugging/instrumentation of the live process.

This boundary must be explicit.

---

## 66. Account Privacy Policy

Phase 2 should persist the minimum public metadata needed.

Avoid collecting/storing:

- Microsoft email;
- real name;
- locale;
- birth date;
- unrelated Xbox profile data;
- unnecessary claims.

If a provider response includes these fields, normalization must discard them unless a concrete
Graphene product requirement exists.

---

## 67. Managed Java Supply-Chain Policy

A managed runtime is executable code downloaded from the network.

Required policy:

- only explicitly configured trusted providers;
- HTTPS production metadata/artifacts;
- trustworthy cryptographic checksum required;
- verify checksum before extraction;
- no provider-supplied shell/postinstall script execution;
- bounded archive parsing;
- staged executable probe;
- provenance stored in `runtime.json`;
- provider identity visible to host in normalized metadata;
- no silent vendor substitution if policy requires a specific vendor.

Future signature verification may be added as an additional capability.

---

## 68. Java Provider Capability Model

A provider capability model prevents hard-coded provider-name branching.

Potential capabilities:

```text
RELEASE_LOOKUP
SHA256
JRE
JDK
X86_64
AARCH64
X86
WINDOWS
LINUX
MACOS
```

The exact representation may be bitflags or structured capability data.

Service code should ask whether the provider can satisfy a `ManagedJavaRequest`, not compare
provider names.

---

## 69. Managed Runtime Updates

Phase 2 should not automatically replace a working committed runtime merely because a newer patch is
available.

Baseline semantics:

- a committed runtime is immutable;
- a new release installs as a new runtime identity;
- selection policy may choose the newest compatible managed runtime;
- removal of old runtimes can be later garbage-collection work.

This preserves reproducibility and rollback potential.

---

## 70. Runtime Deduplication

Duplicate managed runtime requests should converge.

Within one engine:

```text
same normalized release identity
        |
        v
shared keyed install gate
```

Across processes/engines sharing a data root, final publication must remain safe through filesystem
semantics.

The same archive should also naturally deduplicate through the content-addressed artifact cache.

---

## 71. Account Repository Consistency

Account metadata and secret store are two separate persistence systems.

Phase 2 must define consistency behavior.

### 71.1 Create Microsoft Account

Recommended ordering:

1. authenticate fully;
2. validate profile/entitlement;
3. persist refresh credential securely;
4. persist public account record as `Ready`;
5. if public metadata write fails, best-effort delete newly created secret;
6. report structured persistence failure.

Alternative ordering is acceptable only with equivalent cleanup/recovery guarantees.

### 71.2 Startup Reconciliation

On account listing/read:

- public metadata with missing required secret becomes `RequiresReauthentication` or equivalent;
- orphan secret records should not be enumerated as accounts;
- optional maintenance APIs may clean orphan secrets.

Do not panic or silently fabricate a ready session.

---

## 72. Account Reauthentication

Reauthentication should preserve stable `AccountId` when it represents the same local account.

After successful login:

- compare normalized Minecraft profile identity with existing account;
- if it matches, replace/rotate credential and refresh metadata;
- if it differs, reject or require an explicit "replace account identity" operation.

This prevents accidental account identity swapping under an existing `AccountId`.

---

## 73. Account Removal Recovery

Removal spans secret storage and public metadata.

Documented result states should make recovery possible.

Recommended strategy:

1. mark/remove public account through atomic repository operation only after secret deletion
   succeeds, or
2. use a tombstone/recovery record if backend semantics require it.

The simplest safe Phase 2 behavior is to fail removal if the secret cannot be deleted, rather than
pretend the account is fully removed.

Offline account removal has no secret dependency.

---

## 74. Authentication Fixture Strategy

All normal CI auth tests must use a deterministic local fixture provider.

Fixtures should model:

```text
device-code success
authorization_pending
slow_down
device-code expiration
user denial
refresh success
refresh rotation
refresh rejection
Xbox failure
XSTS failure
Minecraft login failure
entitlement missing
profile missing
malformed JSON
oversized response
unexpected status
network interruption
```

Fake secrets must be obvious fixture values and secret-redaction tests must assert they never appear
in debug/error output.

---

## 75. Managed Java Fixture Strategy

Frozen fixtures should include:

```text
release metadata:
  Java 8
  Java 17
  Java 21
  matching x86_64
  matching aarch64
  unsupported architecture
  missing checksum
  malformed release

archives:
  valid ZIP runtime
  valid tar.gz runtime
  traversal entry
  absolute path
  symlink/hardlink attack
  oversized entry
  corrupt compressed data
  valid fake java executable/script fixture
```

Real Java binaries are not required for normal CI.

The existing fake-Java testing approach should be extended rather than replaced.

---

## 76. Unit Test Matrix — Auth Domain

Required unit tests:

- `AccountId` round-trip;
- account kind serialization;
- account state validation;
- bounded display-name validation;
- offline account creation;
- stable offline UUID persistence;
- auth interaction redaction;
- auth session redaction;
- secret key derivation from AccountId;
- provider capability behavior;
- refresh-policy state transitions;
- account-record schema validation;
- provider DTOs do not serialize into domain records.

---

## 77. Unit Test Matrix — Microsoft Provider

Required deterministic provider tests:

- device-code response normalization;
- polling pending behavior;
- slow-down interval handling;
- expiry handling;
- cancellation handling;
- token success normalization;
- refresh rotation;
- Xbox response normalization;
- XSTS response normalization;
- Minecraft service login normalization;
- entitlement response;
- profile response;
- malformed responses;
- missing required fields;
- bounded string/body handling;
- redaction.

---

## 78. Unit Test Matrix — Secret Storage

Required tests:

- put/get/delete;
- missing record;
- atomic replacement;
- encrypted-file round-trip;
- wrong unlock secret;
- corrupted ciphertext;
- truncated file;
- schema mismatch;
- secret not visible in debug output;
- secure fallback never writes plaintext;
- account metadata file contains no fixture token;
- in-memory test backend is opt-in;
- platform backend error normalization.

Platform keyring integration may require conditional/integration tests beyond hermetic CI depending
on runner capabilities.

---

## 79. Unit Test Matrix — Managed Java

Required tests:

- request normalization;
- platform mapping;
- provider release normalization;
- release selection determinism;
- checksum requirement;
- managed runtime ID behavior;
- plan validation;
- runtime descriptor serialization;
- local vs managed selection ordering;
- diagnostics;
- staged probe mismatch;
- inventory validation.

---

## 80. Integration Test Matrix — Microsoft Login

Local fixture integration test:

```text
Graphene
  |
  v
begin login
  |
  v
fixture device code
  |
  v
pending -> success
  |
  v
Xbox -> XSTS -> Minecraft
  |
  v
entitlement/profile
  |
  v
secret store
  |
  v
public account repository
  |
  v
launch_session(account_id)
```

Assertions:

- account is persisted;
- refresh credential is in secret backend only;
- account JSON contains no token;
- session contains required launch values;
- redacted representations contain no token;
- operation stages are observable.

---

## 81. Integration Test Matrix — Refresh

Cases:

1. stored refresh credential -> successful refreshed session;
2. rotated refresh credential replaces old value;
3. temporary network error leaves account `Ready`;
4. refresh rejected -> `RequiresReauthentication`;
5. secret missing -> structured reauth/secret error;
6. profile identity mismatch -> reject update;
7. cancellation before persistence -> old account/credential remain valid.

---

## 82. Integration Test Matrix — Offline Account

Cases:

- create;
- list/get;
- launch-session generation;
- restart engine and preserve same UUID;
- remove;
- invalid names;
- duplicate display names remain distinguishable by `AccountId`;
- no secret-store writes occur.

---

## 83. Integration Test Matrix — Managed Java Install

Local fixture flow:

```text
JavaRequirement
    |
    v
release metadata fixture
    |
    v
ManagedJavaInstallPlan
    |
    v
verified Artifact download
    |
    v
safe extraction
    |
    v
fake Java probe
    |
    v
runtime.json
    |
    v
transactional commit
```

Assertions:

- invalid archive never commits;
- hash mismatch never extracts;
- cancellation never commits;
- repeated request reuses cache;
- repeated committed runtime is reused;
- probe failure never commits;
- compatible committed runtime becomes `JavaRuntime`.

---

## 84. Integration Test Matrix — Java Selection

Required cases:

```text
explicit compatible local -> explicit wins
explicit incompatible -> structured failure
compatible local + managed -> documented default wins
no local + compatible managed -> managed wins
no runtime + allow install -> install managed
no runtime + no side effects -> structured not-found + diagnostics
wrong architecture managed -> rejected
wrong major managed -> rejected
corrupt managed descriptor -> ignored/diagnosed according to policy
```

---

## 85. Secret Leak Regression Test

Create a single unmistakable fixture secret set:

```text
PHASE2_REFRESH_TOKEN_DO_NOT_PRINT
PHASE2_ACCESS_TOKEN_DO_NOT_PRINT
PHASE2_DEVICE_CODE_DO_NOT_PRINT
PHASE2_XSTS_TOKEN_DO_NOT_PRINT
PHASE2_MC_TOKEN_DO_NOT_PRINT
```

Assert those strings do not appear in:

- `Debug` output;
- `Display` output;
- `GrapheneError`;
- `ErrorSummary`;
- operation events;
- account JSON;
- runtime JSON;
- snapshots;
- tracing capture used by tests;
- generated diagnostic bundles if any export helper exists.

This should be a named acceptance test, not an informal review.

---

## 86. Architecture Tests

Expand `scripts/check_architecture.py` and/or `tests/architecture.rs`.

Required checks:

- `graphene-auth` has no network/storage/service/launch/UI dependency;
- `graphene-java` has no provider/network/storage/UI dependency;
- `graphene-launch` has no auth/provider dependency;
- provider DTO directories are private;
- root crate does not re-export provider DTOs;
- no Tauri/Slint dependency;
- no direct Reqwest use outside approved network/provider adapters;
- crate-root `lib.rs` files remain thin;
- Phase 2 service modules exist outside `lib.rs`;
- no cycle introduced.

A practical source-size threshold for `lib.rs` may be enforced if it is robust enough for the
repository. Prefer semantic checks over arbitrary line-count-only rules, but a conservative size
guard can prevent obvious regressions.

---

## 87. Snapshot Tests

Stable snapshots should cover normalized public plans/models only.

Recommended snapshots:

```text
microsoft_account_public_record.json
auth_session_redacted.json
managed_java_release.json
managed_java_install_plan.json
managed_runtime_descriptor.json
java_diagnostics.json
```

Never snapshot real secret-bearing DTOs.

All secret fields must appear only as explicit redaction markers if included in a redacted snapshot.

---

## 88. Property/Fuzz Testing

Priority Phase 2 fuzz/property targets:

- account JSON parser;
- encrypted secret-store file format;
- Microsoft provider JSON normalization;
- Java provider JSON normalization;
- ZIP/tar runtime archive path handling;
- runtime descriptor parser.

Security-sensitive parsers should have explicit size limits before fuzzing so fuzz results reflect
real production bounds.

---

## 89. Cross-Platform CI

Required normal CI:

```text
Linux x86_64
Windows x86_64
macOS
```

Where runners permit:

- account repository tests;
- encrypted fallback tests;
- managed archive extraction;
- fake Java probe;
- selection tests;
- architecture tests;
- secret-redaction tests.

Native OS keyring availability may be inconsistent on headless CI.

Therefore:

- keyring abstraction must be testable through deterministic fake backends;
- real keyring integration may have opt-in platform tests;
- lack of a desktop credential service on a CI runner must not force insecure fallback.

---

## 90. Real Microsoft Authentication Smoke Test

Implementation must add `docs/PHASE_2_AUTH_SMOKE_TEST.md`.

The smoke test should be opt-in because it requires a distributor-owned Microsoft application
registration and a real eligible Minecraft account.

Procedure should verify:

1. create clean data root;
2. configure production Microsoft auth;
3. begin device-code login;
4. present verification URI/code externally;
5. finish login;
6. verify account public metadata;
7. verify secret is stored only in configured secure backend;
8. restart Graphene;
9. request a launch session from persisted account;
10. verify refresh path works without reentering credentials;
11. generate redacted `LaunchPlan`;
12. launch the Phase 1 Vanilla fixture/real installation as appropriate;
13. verify no secret appears in logs;
14. revoke/remove account and verify subsequent refresh fails appropriately.

The execution record must contain no secret values.

---

## 91. Real Managed Java Smoke Test

Implementation must add `docs/PHASE_2_JAVA_SMOKE_TEST.md`.

Procedure:

1. clean Graphene data root;
2. ensure no compatible local Java is selected by using an explicit test policy;
3. resolve a pinned Java major through production provider;
4. inspect normalized release;
5. inspect install plan;
6. download/verify archive;
7. extract and probe;
8. commit runtime;
9. restart Graphene;
10. rediscover committed managed runtime;
11. use it to build a Phase 1 launch plan;
12. optionally launch a real pinned Vanilla instance;
13. repeat ensure/install and confirm cache/runtime reuse.

Record provider, Java version, platform/architecture, digest, and runtime ID.

---

## 92. Dependency Additions Policy

Phase 2 may require dependencies for:

- keyring/credential vault access;
- authenticated encryption;
- password-based key derivation;
- zeroization;
- tar/gzip archive support;
- time/date handling.

Each new dependency must answer:

1. Which bounded context owns it?
2. Is it maintained?
3. Does it introduce native/system dependencies?
4. Does it work on Windows/Linux/macOS?
5. Can it panic on malformed untrusted input?
6. Does it expose secret values through `Debug`?
7. Does it add a UI dependency?
8. Does it complicate static/dynamic distribution?
9. Is a smaller existing dependency already sufficient?

Pinning exact third-party versions belongs to implementation/Cargo review, not this planning
document.

---

## 93. Microsoft Library Decision

Official Microsoft documentation generally recommends supported Microsoft authentication libraries
when available.

Graphene is a Rust library and Phase 2 must evaluate whether an official, appropriate Rust library
exists for the exact required desktop/device-flow scenario at implementation time.

If no suitable supported Rust library exists, a narrow protocol implementation may be used under
`graphene-providers::microsoft`, with:

- strict endpoint configuration;
- fixture coverage;
- bounded DTO parsing;
- explicit protocol tests;
- no leakage into the domain.

This decision should be recorded in an ADR during implementation.

---

## 94. Managed Java Provider ADR

The choice of the first managed-Java distribution is an adapter decision.

An ADR should record:

- chosen provider;
- why it satisfies supported platforms;
- checksum availability;
- archive formats;
- licensing/distribution considerations;
- why provider-specific DTOs stay isolated;
- how a second provider would be added.

The domain must remain vendor-neutral regardless of the initial adapter.

---

## 95. Secure-Store ADR

A Phase 2 ADR should record:

- OS keyring abstraction;
- fallback policy;
- encryption/KDF choices if fallback exists;
- unlock-secret responsibility;
- behavior when no secure store is available;
- threat model;
- test strategy.

No code should make an implicit security downgrade not represented in that ADR.

---

## 96. Public API Stability Strategy

### High-Stability Types

Likely high-stability:

```text
AccountId
Account
AccountKind
AccountProfile
AccountState
OfflineAccountSpec
ManagedJavaRuntime
ManagedJavaRelease
```

### Medium-Stability Types

May remain non-exhaustive:

```text
AuthInteraction
AuthProviderCapabilities
JavaDistributionCapabilities
secret-store policy
provider config
managed Java policy
```

### Private Types

Must remain private:

```text
Microsoft OAuth DTOs
Xbox DTOs
XSTS DTOs
Minecraft auth DTOs
Adoptium/provider DTOs
keyring library error types
AEAD implementation types
KDF implementation types
archive parser entry types
```

---

## 97. Compatibility with Phase 3 — Loaders

Phase 3 loaders must remain unrelated to account protocol and Java distribution selection.

Expected convergence:

```text
Loader-composed ResolvedMinecraft
        |
        +--> JavaRequirement
        |
        v
Phase 2 Java selection/ensure
```

If a loader requires a different Java major, it should express that through the normalized resolved
Minecraft/component requirement, not by calling a Java provider directly.

Authentication remains a separate supplier of `LaunchSession`.

---

## 98. Compatibility with Phase 4 — Instance Engine

Phase 4 will add:

- per-instance account selection;
- per-instance Java override;
- richer configuration inheritance;
- repair/locking.

Phase 2 must therefore avoid baking global "default account" or "default Java" assumptions into
domain types.

Phase 2 service APIs may accept explicit IDs/policies now.

Future Phase 4 configuration can choose them later.

---

## 99. Compatibility with Future Authentication Providers

Later Yggdrasil/authlib-injector support should implement `AuthProvider`-style contracts and still
produce normalized account/session data.

Phase 2 must not define public account fields such as:

```text
microsoft_refresh_token
xsts_token
```

because those would prevent provider-neutral evolution.

---

## 100. Compatibility with Future Java Providers

A second Java distribution must require:

```text
new provider adapter
```

not:

```text
new branches throughout JavaService
```

The selection/install pipeline consumes `ManagedJavaRelease`/`Artifact`/`ManagedJavaRuntime`
regardless of provider.

---

## 101. Phase 2 Workstreams

### Workstream 1 — Phase 1 Gate and Architecture Activation

- obtain Phase 1 green executable evidence;
- complete real Phase 1 smoke record;
- add `graphene-auth`;
- extend workspace dependencies;
- update architecture checker;
- add new typed IDs/error kinds;
- keep crate roots thin.

**Exit:** empty auth crate/domain skeleton compiles with correct dependency direction.

### Workstream 2 — Account Domain and Persistence

- account models;
- offline model;
- account repository port;
- account JSON schema;
- atomic account storage;
- list/get/remove;
- fixture tests.

**Exit:** offline accounts survive engine restart and require no secret store.

### Workstream 3 — Secret Store

- secret-store interface;
- platform secure-store adapter;
- encrypted fallback if approved;
- test in-memory backend;
- builder configuration;
- redaction tests;
- failure normalization.

**Exit:** a fixture refresh credential can survive restart through a secure backend and never
appears in public metadata/log output.

### Workstream 4 — Microsoft Provider

- config/policy;
- device-code DTOs;
- token DTOs;
- Xbox/XSTS DTOs;
- Minecraft auth/entitlement/profile DTOs;
- normalization;
- fixture server;
- provider errors.

**Exit:** fixture Microsoft login reaches a normalized `AuthSession` with all DTOs private.

### Workstream 5 — Account Service and Refresh

- interactive login operation;
- persistence orchestration;
- refresh workflow;
- refresh rotation;
- reauthentication state;
- per-account gates;
- session cache;
- account-to-`LaunchSession`.

**Exit:** persisted fixture account produces a fresh launch session after engine restart.

### Workstream 6 — Managed Java Domain and Provider

- provider-neutral models;
- provider capabilities;
- reference provider;
- release selection;
- archive artifact mapping;
- fixture metadata.

**Exit:** Java requirement resolves deterministically into a verifiable managed release.

### Workstream 7 — Managed Java Installation

- plan;
- archive handling;
- staging;
- extraction;
- probe;
- descriptor;
- commit;
- inventory;
- keyed gates.

**Exit:** fixture managed runtime installs transactionally and normalizes to `JavaRuntime`.

### Workstream 8 — Java Selection and Diagnostics

- extend selection;
- managed inventory;
- explicit no-side-effect select;
- explicit ensure/install path;
- diagnostics;
- launch integration.

**Exit:** incompatible/no-Java scenario can be solved through explicit managed install without
changing launch-plan construction.

### Workstream 9 — Hardening and Sign-Off

- secret leak regression;
- cancellation matrix;
- malformed provider responses;
- archive attacks;
- cross-platform CI;
- real Microsoft smoke;
- real managed Java smoke;
- API/security docs;
- ADRs.

**Exit:** Phase 2 checklist completely green.

---

## 102. Recommended Implementation Order

Within workstreams, use this order:

```text
1. Phase 1 gate
2. graphene-auth domain
3. account storage
4. secret-store abstraction/test backend
5. offline accounts
6. Microsoft fixture protocol adapter
7. Microsoft login orchestration
8. refresh + persistence
9. LaunchSession conversion
10. managed-Java domain
11. Java provider fixture
12. managed-Java plan/install
13. managed runtime inventory
14. selection/ensure integration
15. diagnostics
16. security hardening
17. real smoke tests
```

This order produces useful deterministic vertical slices early.

---

## 103. Review Checklist — Architecture

Before Phase 2 sign-off:

- [ ] `graphene-auth` exists and owns account/auth domain.
- [ ] `graphene-auth` does not depend on network/storage/service/launch/UI.
- [ ] `graphene-launch` does not depend on auth/providers.
- [ ] Microsoft DTOs remain private in providers.
- [ ] Java distribution DTOs remain private in providers.
- [ ] service owns orchestration only.
- [ ] root `lib.rs` files are thin.
- [ ] dependency graph remains acyclic.
- [ ] architecture checker passes.

---

## 104. Review Checklist — Authentication

- [ ] Microsoft config requires distributor-owned client identity.
- [ ] device-code interaction is UI-independent.
- [ ] polling is bounded/cancellable.
- [ ] provider stages are normalized.
- [ ] entitlement is verified.
- [ ] profile is verified.
- [ ] public account metadata is provider-neutral.
- [ ] refresh credential is the only normal persistent auth secret.
- [ ] refresh rotation is safe.
- [ ] reauthentication state is explicit.
- [ ] offline account behavior is explicit.
- [ ] both account kinds produce `LaunchSession`.

---

## 105. Review Checklist — Secrets

- [ ] no plaintext credential persistence.
- [ ] secure-store unavailability fails securely.
- [ ] keyring work is behind a blocking boundary if required.
- [ ] fallback encryption is authenticated.
- [ ] fallback key is not persisted beside ciphertext.
- [ ] test backend is explicit.
- [ ] account JSON contains no secret.
- [ ] debug/display are redacted.
- [ ] errors/events are redacted.
- [ ] leak regression test passes.

---

## 106. Review Checklist — Managed Java

- [ ] provider-neutral Java distribution contract exists.
- [ ] one concrete provider proves the contract.
- [ ] remote runtime has trustworthy integrity.
- [ ] common artifact pipeline is reused.
- [ ] runtime archive parsing is bounded/safe.
- [ ] staging is isolated.
- [ ] staged Java is probed.
- [ ] failed probe does not commit.
- [ ] runtime descriptor is schema-versioned.
- [ ] committed runtime becomes existing `JavaRuntime`.
- [ ] selection policy is deterministic.
- [ ] auto-install side effects are explicit.
- [ ] Java diagnostics are structured.

---

## 107. Phase 2 Exit Checklist

Phase 2 may be marked complete only when every item below is checked with executable evidence.

### Architecture

- [ ] Phase 1 final gate is green.
- [ ] Phase 1 real Vanilla smoke is recorded as passed.
- [ ] `graphene-auth` is activated.
- [ ] dependency graph is acyclic.
- [ ] architecture checker includes Phase 2 rules.
- [ ] no UI dependency exists.
- [ ] crate roots remain thin.
- [ ] no provider DTO escapes `graphene-providers`.

### Account Domain

- [ ] `AccountId` is stable/typed.
- [ ] account public model exists.
- [ ] account schema is versioned.
- [ ] account repository is atomic.
- [ ] account listing/get works.
- [ ] offline create works.
- [ ] offline UUID remains stable across restart.
- [ ] offline account requires no secret store.
- [ ] account removal is tested.

### Microsoft Authentication

- [ ] application client identity is explicit configuration.
- [ ] device authorization works through fixtures.
- [ ] authorization pending/slow-down semantics are tested.
- [ ] cancellation is tested.
- [ ] expiration/denial are tested.
- [ ] Xbox stage is normalized.
- [ ] XSTS stage is normalized.
- [ ] Minecraft services login is normalized.
- [ ] entitlement is validated.
- [ ] profile is validated.
- [ ] real Microsoft smoke procedure exists.

### Refresh

- [ ] persisted account refreshes after engine restart.
- [ ] refresh token rotation is tested.
- [ ] temporary failure preserves account state.
- [ ] rejected credential marks reauthentication required.
- [ ] identity mismatch is rejected.
- [ ] concurrent refresh is serialized per account.

### Secret Storage

- [ ] production secure-store abstraction exists.
- [ ] no plaintext fallback exists.
- [ ] OS secure-store adapter exists for supported platforms or platform limitation is explicitly
  documented.
- [ ] encrypted fallback, if included, passes threat-model/security review.
- [ ] fixture in-memory store exists.
- [ ] secret-store failure is structured.
- [ ] secret delete is verified during account removal.
- [ ] secret leak regression passes.

### Launch Integration

- [ ] Microsoft account produces existing `LaunchSession`.
- [ ] offline account produces existing `LaunchSession`.
- [ ] existing explicit `LaunchRequest` still works.
- [ ] launch argument logic has no provider branch.
- [ ] `graphene-launch` has no auth dependency.
- [ ] redacted launch snapshot remains secret-free.

### Managed Java Domain

- [ ] provider-neutral release model exists.
- [ ] provider-neutral provider interface exists.
- [ ] managed runtime identity exists.
- [ ] runtime descriptor is versioned.
- [ ] inventory is deterministic.

### Managed Java Provider

- [ ] one production provider adapter exists.
- [ ] provider DTOs are private.
- [ ] platform/architecture mapping is tested.
- [ ] verifiable checksum is required.
- [ ] malformed/missing-checksum releases are rejected.
- [ ] fixture provider tests are deterministic.

### Managed Java Installation

- [ ] install plan exists before mutation.
- [ ] shared artifact pipeline is reused.
- [ ] ZIP handling is safe where used.
- [ ] tar.gz handling is safe where used.
- [ ] traversal/link/archive bomb cases are tested.
- [ ] extraction does not block async I/O workers.
- [ ] staged runtime is probed.
- [ ] incompatible staged runtime is rejected.
- [ ] commit is transactional.
- [ ] cancellation leaves no committed partial runtime.
- [ ] repeated install reuses verified archive/runtime.

### Java Selection and Diagnostics

- [ ] explicit override behavior remains stable.
- [ ] local selection remains deterministic.
- [ ] managed selection is deterministic.
- [ ] no-side-effect select is distinct from ensure/install.
- [ ] no compatible Java yields structured diagnostics.
- [ ] architecture mismatch yields structured diagnostic.
- [ ] managed-provider unavailable yields structured diagnostic.
- [ ] real managed Java smoke procedure exists.

### Security

- [ ] all auth response bodies are bounded.
- [ ] all production auth endpoints require HTTPS.
- [ ] credential POST retry behavior is explicitly safe.
- [ ] tokens do not appear in URLs/logs/errors/events.
- [ ] account metadata contains no secret.
- [ ] runtime archive supply-chain policy is enforced.
- [ ] no downloaded postinstall script is executed.
- [ ] blocking keyring/KDF/archive work is isolated.
- [ ] Phase 2 security review is completed.

### Tests and CI

- [ ] auth unit matrix passes.
- [ ] secret-store matrix passes.
- [ ] Microsoft fixture integration passes.
- [ ] refresh matrix passes.
- [ ] offline account matrix passes.
- [ ] managed Java unit matrix passes.
- [ ] managed Java install integration passes.
- [ ] Java selection matrix passes.
- [ ] secret leak regression passes.
- [ ] Windows CI passes.
- [ ] Linux CI passes.
- [ ] macOS CI passes.
- [ ] `cargo fmt` passes.
- [ ] `cargo check` passes.
- [ ] `cargo test` passes.
- [ ] `cargo clippy -D warnings` passes.
- [ ] `cargo doc` passes.
- [ ] architecture checker passes.
- [ ] real Microsoft auth smoke passes.
- [ ] real managed Java smoke passes.

---

## 108. Documentation Deliverables During Implementation

Phase 2 implementation must produce/update:

```text
docs/PHASE_2_IMPLEMENTATION_PLAN.md    # this normative plan
docs/PHASE_2_API.md                    # actual public behavior
docs/PHASE_2_SECURITY_REVIEW.md        # completed security review
docs/PHASE_2_AUTH_SMOKE_TEST.md        # real auth sign-off
docs/PHASE_2_JAVA_SMOKE_TEST.md        # real managed Java sign-off
docs/ROADMAP.md
README.md
CONTRIBUTING.md                        # only if new invariant is added
docs/adr/<phase2-auth-decision>.md
docs/adr/<phase2-secret-store>.md
docs/adr/<phase2-managed-java>.md
```

The API document must describe what was actually implemented, not merely repeat this plan.

---

## 109. Definition of Done

Phase 2 is done when this statement is demonstrably true:

> From a persisted Graphene data root, a host can create an offline account or authenticate a real
> Microsoft-backed Minecraft account through a UI-independent operation, restart the engine,
> obtain a fresh ephemeral `LaunchSession` from that persisted account without implementing OAuth,
> Xbox, Minecraft entitlement/profile, refresh, or secret-storage logic itself, and use that
> session unchanged with the existing Phase 1 launch planner. If no compatible local Java exists,
> the host can explicitly ask Graphene to resolve, securely download, verify, transactionally
> install, probe, inventory, and select a compatible managed runtime through a provider-neutral Java
> API, producing the same existing `JavaRuntime` consumed by Phase 1. Persistent secrets never
> appear in ordinary files or observable diagnostics, long operations remain cancellable, and all
> provider/platform details stay behind their bounded-context adapters.

Phase 2 is **not** done merely because:

- one hard-coded Microsoft token can launch Minecraft;
- a refresh token is written to JSON;
- one machine has Java preinstalled;
- a downloaded JDK is unzipped directly into `shared/runtimes`;
- service code calls Microsoft endpoints directly;
- `lib.rs` files contain the full implementation;
- tests depend on public authentication services.

The phase is done only when authentication and managed Java are reusable, provider-isolated, secure,
deterministic where possible, testable offline, and ready for loader composition in Phase

3.

---

## 110. Phase 2 Architectural North Star

```text
                             Graphene Facade
                                   |
                             Service Wiring
                     +-------------+-------------+
                     |                           |
                     v                           v
               AccountService               JavaService
                     |                           |
          +----------+----------+       +--------+---------+
          |                     |       |                  |
          v                     v       v                  v
     graphene-auth          storage  graphene-java    Artifact Pipeline
          |                     |       |                  |
          |                     |       |                  |
          v                     v       v                  |
   AuthProvider port      SecretStore  JavaDistribution   |
          |                     |       Provider port      |
          |                     |       |                  |
          +----------+----------+       +--------+---------+
                     |                           |
                     v                           v
              Concrete adapters            Concrete adapter
              Microsoft/Xbox/MC              Java distro
                     |                           |
                     +-------------+-------------+
                                   |
                                   v
                               Network
```

Both branches converge into stable Phase 1 values:

```text
Account/Auth                       Managed Java
    |                                  |
    v                                  v
LaunchSession                      JavaRuntime
    |                                  |
    +----------------+-----------------+
                     |
                     v
               LaunchRequest
                     |
                     v
                LaunchPlan
                     |
                     v
                 Process
```

The defining Phase 2 boundary is:

> Authentication supplies `LaunchSession`; managed Java supplies `JavaRuntime`; neither is allowed
> to rewrite the launch engine that consumes them.
