# Phase 2 Security Review

This review records the Phase 2 implementation candidate against the normative security
requirements. It distinguishes source-level controls from executable validation. The sandbox does
not contain a Rust toolchain, so Cargo tests, Clippy, Rustdoc, formatting, cross-platform CI, and
real-network smoke tests are **not claimed as passed**.

## Authentication Boundary

- `graphene-auth` has no dependency on `graphene-network`, `graphene-storage`, `graphene-service`,
  or
  `graphene-launch`.
- Microsoft/Xbox/XSTS/Minecraft protocol DTOs are private to `graphene-providers`.
- `graphene-service` consumes only normalized `AuthProvider`/`AuthSession` values and performs the
  sole account-to-existing-`LaunchSession` conversion.
- `graphene-launch` remains independent from authentication and providers and contains no
  account-provider branch.
- Microsoft application identity is explicit configuration. No production OAuth client ID is
  hard-coded or borrowed from another launcher.

## Device Authorization and Network Safety

- Production Microsoft identity endpoints require HTTPS; fixture HTTP is restricted to explicit
  loopback configuration.
- Auth protocol response bodies are bounded before deserialization.
- Device polling respects the normalized provider interval, `authorization_pending`, `slow_down`,
  expiry, denial, and cancellation checkpoints. It never busy-polls.
- Credential-bearing form/JSON POST and bearer requests use the shared Graphene network client but a
  no-redirect protocol client, preventing redirect forwarding of request bodies or bearer headers.
- Authentication POSTs are not sent through the generic artifact retry loop.
- Transport errors are normalized and do not copy request URLs, bodies, tokens, or raw provider
  response data into the public error contract.

## Secret Classification and Persistence

Persistent public account files contain `AccountId`, account kind, normalized profile, state, and a
schema version only. The following values are not serializable through the account model and are not
written to account JSON:

- Microsoft access token;
- Microsoft refresh credential;
- device code/user code;
- Xbox user token;
- XSTS token;
- Minecraft access token.

`RefreshCredential`, `AuthInteraction`, `AuthChallenge`, `AuthSession`, and `SensitiveString`
formatting redact sensitive fields. Secret-record identity uses a Graphene namespace plus record
kind and `AccountId`, not display name or email.

The only normal Microsoft persistent secret is the refresh credential. Public metadata and secret
storage use recoverable ordering: a new Microsoft account is fully authenticated first, the refresh
credential is stored, then public `Ready` metadata is atomically persisted; public persistence
failure attempts secret cleanup. Reauthentication restores the prior credential if cancellation or
public persistence failure occurs after writing the replacement. Refresh rotation differs
deliberately:
cancellation before secure replacement leaves the old credential untouched, while cancellation after
a durable replacement keeps the new credential because the provider may already have invalidated the
old one.

Missing required secret state is never reported as ready. Account removal deletes the Microsoft
secret before public metadata; secret deletion failure leaves the public record intact.

## Secure-Store Decision and Limitation

`SecretStore` is a provider-neutral port and all service calls to it execute on blocking workers.
The deterministic in-memory backend is explicitly named `new_for_tests`. The default backend is
`UnavailableSecretStore`, returning structured `AUTH_SECRET_STORE_UNAVAILABLE` failures.

This implementation candidate deliberately includes **no plaintext fallback** and **no encrypted
file fallback**. It also does **not bundle an OS credential-vault adapter**. Production integration
must inject a secure `SecretStore` implementation; otherwise Microsoft persistence fails safely.
This is an explicit platform/composition limitation, not a silent downgrade. ADR-0007 records the
threat model and revisit condition.

Because no encrypted-file fallback exists, no KDF/encryption-key material is introduced or stored
beside ciphertext.

## Refresh and Race Behavior

- Refresh/launch-session work is serialized per `AccountId` by an engine-local async gate.
- Removal also acquires the same gate, so it cannot delete a credential midway through a refresh
  persistence section.
- Reauthentication performs the interactive provider chain before taking the account gate, then
  serializes identity verification and persistence; a concurrent refresh can complete while the user
  interaction is pending, but commits cannot race.
- Confirmed refresh rejection and profile-identity mismatch transition the public account to
  `RequiresReauthentication`.
- Temporary provider/network failure leaves the prior public readiness state intact.
- A replacement refresh credential is durably written before public account metadata is updated.

## Managed Java Supply Chain

- `graphene-java` owns provider-neutral models and has no provider/network/storage dependency.
- Adoptium DTOs/API mapping stay in `graphene-providers`.
- The adapter requires SHA-256 metadata and an HTTPS artifact URL in production. Loopback HTTP is
  allowed only through explicit fixture provider configuration.
- Managed archives become ordinary Graphene `Artifact` values and therefore use the existing shared
  acquisition pipeline for proxy policy, bounded concurrency, streaming, retries, cache reuse,
  duplicate gates, cancellation, and cryptographic verification.
- Graphene does not execute MSI/PKG/DEB/RPM installers, provider scripts, or arbitrary post-install
  hooks. Phase 2 consumes archive distributions only.

## Runtime Archive and Filesystem Safety

Managed ZIP extraction reuses the shared bounded ZIP/DEFLATE byte codec and the Phase 1 extraction policy. Managed tar.gz adds
bounded gzip/DEFLATE and tar parsing. Both paths reject:

- absolute paths and platform prefixes;
- `..`, empty-component, and backslash traversal;
- symlinks and hardlinks;
- unsupported special files;
- excessive entry count;
- oversized entries;
- excessive total expanded size;
- implausible compression/expansion ratios;
- corrupt compression, CRC, or tar-header checksums.

Frozen fixtures include valid ZIP/tar.gz plus traversal, absolute, backslash, symlink, hardlink,
oversized declaration, expansion-bomb policy, and corrupt-input cases. These tests are present but
cannot be executed in this sandbox because Cargo is unavailable.

Extraction, managed-runtime filesystem inspection, descriptor work, and publication run through
blocking-task boundaries where potentially expensive. Runtime staging is isolated under
`shared/runtimes/.staging-<operation-id>`.

A verified archive is not enough to publish a runtime. Graphene locates an ordinary staged Java
executable, sets only the necessary executable bit on Unix, uses the existing bounded Java probe,
verifies normalized major/architecture compatibility, writes/re-reads a schema-versioned descriptor,
checks cancellation, seals cancellation, and only then performs create-only publication. Failure
before publication cleans staging. A committed runtime is re-probed before selection.

## Secret Leak Regression

The Phase 2 fixture suite uses unmistakable `PHASE2_*_DO_NOT_PRINT` sentinel values defined in test
code.

Tests assert sensitive values do not appear in auth/session debug formatting, public account JSON,
operation-event rendering, or managed runtime descriptors. The fixture backend verifies the refresh
credential is stored separately. Because Rust tests cannot run here, this review records the tests
as implemented, **not passed**.

## Architecture Guard

`python scripts/check_architecture.py` was executed successfully on this worktree. It verifies:

- the `graphene-auth` manifest and exact approved internal dependency graph;
- forbidden auth/java/launch dependency edges;
- acyclicity;
- no Tauri/Slint dependency;
- Reqwest confined to `graphene-network`;
- provider DTO-like names do not leak outside `graphene-providers` and DTO declarations are not
  public or crate-public;
- every workspace crate root remains a thin facade and below the configured root-size bound.

## Residual Risk / Remaining Acceptance Work

1. A concrete production OS credential-vault adapter is not bundled; production Microsoft
   persistence requires an injected secure backend.
2. Cargo compilation, tests, format, Clippy, Rustdoc, and cross-platform CI have not run in this
   sandbox.
3. The Phase 1 real Vanilla smoke remains `NOT RUN`, so the Phase 2 prerequisite gate is not green.
4. The real Microsoft authentication smoke and real managed-Java provider smoke remain `NOT RUN`.
5. No claim is made that deterministic fixtures substitute for official-service smoke evidence.

## Verification Record

- `cargo fmt --all -- --check` — **NOT RUN TO COMPLETION**: `cargo` is not installed (exit 127).
- `cargo check --workspace --all-targets` — **NOT RUN TO COMPLETION**: `cargo` is not installed
  (exit 127).
- `cargo test --workspace` — **NOT RUN TO COMPLETION**: `cargo` is not installed (exit 127).
- `cargo clippy --workspace --all-targets -- -D warnings` — **NOT RUN TO COMPLETION**: `cargo` is
  not installed (exit 127).
- `cargo doc --workspace --no-deps` — **NOT RUN TO COMPLETION**: `cargo` is not installed (exit
  127).
- `python scripts/check_architecture.py` — **PASS**.
- Phase 1 real Vanilla smoke — **NOT RUN** (see `PHASE_1_SMOKE_TEST.md`).
- Phase 2 Microsoft smoke — **NOT RUN** (see `PHASE_2_AUTH_SMOKE_TEST.md`).
- Phase 2 managed Java smoke — **NOT RUN** (see `PHASE_2_JAVA_SMOKE_TEST.md`).
