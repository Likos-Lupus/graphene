# Phase 2 API — Authentication and Managed Java

This document describes the Phase 2 behavior implemented in this worktree. The normative scope and
exit criteria remain in `PHASE_2_IMPLEMENTATION_PLAN.md`. This worktree is an **implementation
candidate**, not a Phase 2 completion record: the sandbox has no Rust toolchain, the Phase 1 real
Vanilla smoke remains `NOT RUN`, and the Phase 2 real smoke procedures have not been executed.

## Engine Composition

`Graphene::builder(data_root)` remains the UI-independent composition entry point. Phase 2 adds:

- `Graphene::accounts()` for account/authentication orchestration;
- managed-runtime behavior under the existing `Graphene::java()` service;
- `GrapheneBuilder::microsoft_auth(MicrosoftAuthConfig)` for an explicit distributor-owned Microsoft
  application/protocol configuration;
- `GrapheneBuilder::secret_store(Arc<dyn SecretStore>)` for an explicit secure credential backend;
- `GrapheneBuilder::managed_java_provider(AdoptiumProviderConfig)` for the reference managed-Java
  adapter configuration.

The builder defaults to `UnavailableSecretStore`; Graphene does not silently persist credentials in
plaintext. The Microsoft provider is disabled until explicitly configured. The Adoptium managed-Java
adapter uses its HTTPS production API configuration by default.

Provider DTOs and Reqwest values stay private. The root facade exposes Graphene-owned account,
authentication interaction, managed-Java model, configuration, and service types only.

## Accounts

`AccountId` is a Graphene-owned opaque typed ID. It is independent from Microsoft identifiers,
Minecraft UUIDs, email addresses, and display names.

The persisted public account model is:

- `AccountKind::{Microsoft, Offline}`;
- `AccountProfile { display_name, minecraft_uuid }`;
- `AccountState::{Ready, RequiresReauthentication, SecretStoreUnavailable}`;
- `Account { id, kind, profile, state }`.

Public account documents are stored beneath `config/accounts/<account-id>.json` using a versioned
envelope. Storage is atomic and bounded. Repository reads validate both the filename identity and
the deserialized account model before returning it through the service.

`graphene.accounts()` provides:

- `list()`;
- `get(account_id)`;
- `create_offline(OfflineAccountSpec)`;
- `begin_microsoft_login()`;
- `reauthenticate(account_id)`;
- `launch_session(account_id)`;
- `remove(account_id)`.

## Offline Accounts

Offline accounts are local identities, not Microsoft emulation. Names are limited to 1–16 ASCII
letters, digits, or underscore. If a UUID is not supplied, a UUID is generated once when the account
is created and then persisted. Restarting the engine therefore preserves the same profile UUID.

Offline accounts have no secret-store record. `launch_session(account_id)` converts a ready offline
account to the existing `LaunchSession` with the persisted profile identity and legacy/offline
session semantics. This does not claim authenticated online-mode access.

## Microsoft Device Authorization

Concrete Microsoft/Xbox/Minecraft protocol code lives under the private `microsoft` namespace of
`graphene-providers`. `graphene-auth` owns only normalized ports and models.

`begin_microsoft_login()` starts an operation and returns `MicrosoftLoginOperation`. Its
`AuthInteraction::DeviceAuthorization` contains a verification URI plus redacted sensitive user
interaction values, expiry, and poll interval. The host displays the typed interaction; Graphene
does not create browser windows, webviews, or host callbacks.

The provider adapter performs the normalized sequence:

1. request device authorization;
2. poll according to the provider interval and `slow_down` responses without busy-polling;
3. exchange Microsoft identity credentials;
4. authenticate Xbox;
5. authorize XSTS;
6. authenticate Minecraft services;
7. verify entitlement;
8. fetch the Minecraft profile;
9. return normalized `AuthSession` data to `AccountService`.

Production Microsoft identity configuration derives the v2 device-code/token endpoints from an
explicit client ID and tenant. There is no built-in borrowed launcher client ID. Xbox/Minecraft
service endpoints and relying-party values are explicit `MicrosoftServiceEndpoints` configuration;
fixture HTTP is accepted only through `MicrosoftAuthConfig::fixture` and only for loopback URLs.
Credential-bearing protocol requests do not automatically follow redirects and are not placed on the
generic retry loop.

All auth responses are body-bounded. Cancellation is checked before provider stages, during device
poll waiting, and before persistence.

## Secrets and Refresh

`RefreshCredential` and the Phase 1 `SensitiveString` foundation redact formatting and are not
serializable. A Microsoft `AuthSession` keeps the Microsoft/Minecraft access material ephemeral; the
only normal persistent credential is the refresh credential.

The `SecretStore` port is provider-neutral. `SecretRecordIdentity` derives a stable key from the
Graphene namespace, record kind, and `AccountId`. Secret operations are moved to blocking workers by
service orchestration. The deterministic `InMemorySecretStore::new_for_tests()` backend is explicit.
`UnavailableSecretStore` is the safe default when no secure backend is configured.

This candidate intentionally contains no plaintext fallback and no built-in encrypted-file fallback.
It also does not yet bundle a concrete OS keyring adapter; production hosts/distributors must inject
a `SecretStore` implementation. That limitation is recorded in ADR-0007 and the Phase 2 security
review.

For Microsoft launch-session refresh, `AccountService` serializes refresh work per `AccountId`,
loads the stored credential, refreshes Microsoft identity, repeats the Xbox/XSTS/Minecraft chain,
verifies profile identity, securely persists a rotated refresh credential before publishing updated
public metadata, and returns the existing `LaunchSession`.

Confirmed refresh rejection or identity mismatch moves the public account to
`RequiresReauthentication`. Temporary provider/network failures do not invalidate a ready account.
Missing secret records are reconciled to reauthentication; an unavailable secret backend is exposed
as `SecretStoreUnavailable` without fabricating readiness.

Microsoft removal deletes the secret record before deleting public metadata. If secret deletion
fails, public metadata remains rather than reporting a false complete removal.

## Launch Boundary

Authentication does not modify `graphene-launch`. Both account kinds converge in
`graphene-service`:

```text
graphene-auth AuthSession
        |
        v
graphene-service conversion
        |
        v
existing graphene-launch LaunchSession
```

The existing launch placeholder and argv construction code therefore receives the same
`LaunchSession` type and contains no Microsoft/offline provider branch.

## Managed Java Domain

`graphene-java` retains Phase 1 local discovery/probing/selection and adds provider-neutral managed
runtime concepts:

- `ManagedJavaRequest`;
- `ManagedJavaRelease`;
- `ManagedJavaInstallPlan`;
- `ManagedJavaRuntime`;
- `ManagedRuntimeId`;
- `JavaDistributionProvider` and capabilities;
- deterministic committed-runtime selection;
- compatibility diagnostics.

`ManagedJavaRuntime` is schema-versioned and stores the executable as a managed relative path. A
committed runtime identity is immutable; publication is create-only.

## Reference Java Provider

`graphene-providers` contains one private-DTO Adoptium adapter targeting Eclipse Temurin metadata.
It maps Graphene OS/architecture/image requirements to provider values, selects a deterministic
compatible release, requires SHA-256 metadata, permits only ZIP or tar.gz archives, creates a normal
Graphene `Artifact`, and keeps provider DTOs private.

A JRE request may fall back to a JDK release for the same major/platform when the provider returns
no JRE. Unverifiable, unsupported, malformed, or mismatched metadata is rejected instead of silently
substituting an archive.

## Managed Runtime Installation

`JavaService::select_for_instance(instance_id, explicit)` remains side-effect-free. Its order is:

1. explicit Java override;
2. compatible Phase 1 local Java;
3. compatible committed managed Java;
4. structured not-found/incompatible result.

`JavaService::ensure_for_instance` explicitly permits managed installation after local and committed
managed selection fail only with expected not-found/incompatible outcomes.
`JavaService::install_managed(requirement)` is the explicit normalized installation entry point.

Managed installation performs:

1. normalize `ManagedJavaRequest`;
2. resolve a `ManagedJavaRelease`;
3. validate `ManagedJavaInstallPlan` before mutation;
4. serialize installation per managed runtime identity;
5. acquire the archive through the existing `ArtifactService` pipeline;
6. verify the planned SHA-256 result;
7. extract to `shared/runtimes/.staging-<operation-id>/runtime` on a blocking worker;
8. locate the expected `bin/java` or `bin/java.exe` ordinary file;
9. run the existing bounded Phase 1 Java probe;
10. verify major version and architecture compatibility;
11. write and re-read the versioned runtime descriptor;
12. seal cancellation immediately before publication;
13. publish create-only to `shared/runtimes/<managed-runtime-id>/`.

A directory alone is not a valid runtime. Inventory requires a valid descriptor and ordinary
executable. Committed selection probes the executable again before returning the existing
`JavaRuntime`.

The managed archive layer reuses the Phase 1 ZIP/deflate primitives and adds bounded tar.gz support.
It rejects absolute/traversal/backslash/prefix paths, symlinks, hardlinks, special files, excessive
entry count/entry size/total expansion, implausible compression ratios, malformed compression, and
integrity/checksum failures. Downloaded package installer scripts are never executed.

## Diagnostics and Errors

Phase 2 extends stable `ErrorCode` families for authentication/secret/persistence and managed-Java
provider/release/archive/runtime failures. `graphene-auth` exposes normalized account/authentication
diagnostics, while `graphene-java::compatibility_diagnostic` provides safe Java compatibility
parameters such as required/found major, architecture, vendor, and source.

Provider response bodies and raw provider error types do not cross the adapter boundary. Secret
values are excluded from Graphene account JSON, runtime descriptors, operation events, and normal
`Debug`/`Display` formatting by construction and regression fixtures.

## Verification Status

`python scripts/check_architecture.py` passes for this worktree. Cargo format/check/test/Clippy/doc
commands cannot execute because `cargo` is absent from this sandbox. The deterministic Rust tests
are present but are therefore **not recorded as passed** here. The Phase 1 real Vanilla smoke and
both Phase 2 real smoke procedures remain `NOT RUN`.
