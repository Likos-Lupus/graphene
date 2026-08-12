# Phase 1 Security Review

This review records the Phase 1 implementation against the normative security requirements. Items
listed as implemented were re-verified by source inspection and deterministic fixture construction
in this worktree. `python scripts/check_architecture.py` was executed successfully after the
internal modularization and verifies the approved dependency graph, provider DTO boundary,
UI/network boundaries, and thin Phase 1 crate roots. Runtime test execution remains a separate
acceptance gate; this environment does not contain a Rust toolchain, so the Cargo
test/Clippy/doc/format gates and the real Vanilla smoke test are **not recorded as passed here**.

## Remote Metadata and Transport

- Manifest and metadata response bodies have explicit byte limits; semantic count/string/path limits
  are applied after deserialization.
- Unknown JSON fields remain forward-compatible through Serde defaults; unsupported or invalid
  required structures return structured `MINECRAFT_*` failures rather than panicking.
- Production provider endpoints require HTTPS, reject credential-bearing userinfo, and preserve TLS
  validation. Local HTTP is available only through explicit fixture configuration.
- Artifact transport accepts only HTTP/HTTPS sources, rejects userinfo, bounds concurrency/retries,
  and rejects an HTTPS request whose final redirect target downgrades to HTTP.
- Full source URLs are redacted from `Debug`, tracing, and structured error context.
- Normal integration fixtures use an ephemeral local server and do not require public Mojang
  infrastructure.

## Integrity and Acquisition

- Phase 1 does not introduce a second downloader or artifact cache. Installer acquisition uses the
  `ArtifactAcquirer` port and the `graphene-service` adapter delegates to the Phase 0 verified
  `ArtifactService`.
- Expected size/SHA-1/SHA-256 verification remains in the shared Phase 0 acquisition path.
- Existing shared immutable materializations are re-hashed before reuse; invalid materializations
  are replaced only from an already verified acquired cache object.
- Hashing and large copy work are placed on blocking-task boundaries and observe operation
  cancellation cooperatively. Staged install filesystem validation is also delegated to a blocking
  worker after bounded metadata reads.

## Managed Filesystem Boundaries

- Provider-derived file destinations are converted to validated forward-slash managed-relative
  paths. Empty components, `.`, `..`, backslashes, absolute paths, and platform-prefix escapes are
  rejected.
- Materialization scope validation keeps shared artifacts under `shared/` and instance artifacts
  under `.minecraft/` or `.graphene/`.
- Managed directory creation and launch reconstruction reject symbolic-link traversal through
  managed ancestors.
- Final instance publication is create-only/no-replace and happens only after staged validation and
  the operation cancellation seal.
- Failure/cancellation cleanup removes staging state; a target cannot become a valid-looking
  committed instance before the publication boundary.

## Native Archives

The shared filesystem-neutral ZIP/DEFLATE codec plus the Phase 1 native extraction policy are intentionally bounded and reject:

- absolute, parent-traversal, backslash, or otherwise unsafe entry paths;
- symbolic-link and unsupported entry types;
- encryption, multi-disk archives, and ZIP64 forms outside the Phase 1 bounded implementation;
- excessive archive input size, entry count, entry size, total expanded size, filename size, and
  implausible decompression ratio;
- malformed stored/deflate data and CRC mismatches.

Archive extraction executes on a blocking worker and observes cancellation. Frozen fixtures cover
valid deflate, parent traversal, absolute paths, and symlink-shaped entries.

## Java and Process Execution

- Java probing and game launch spawn an executable directly with argv; no shell is used.
- Probe duration and captured output are bounded.
- Game stdout/stderr are drained concurrently. Host-facing event queues are bounded, output can be
  dropped under backpressure, and dropped output is observable without blocking the child.
- Tokio child/process internals remain private to platform/launch implementation boundaries.
- Executable, working-directory, classpath, natives, and persisted managed paths are validated
  before launch. Scalable launch filesystem validation runs on a blocking worker rather than the
  async I/O executor.

## Secrets

- `SensitiveString` redacts `Debug` and `Display` and is intentionally non-serializable.
- `LaunchSession` is ephemeral and is never written to `instance.json` or `install.json`.
- Access-token/client/XUID placeholders are classified as secret argv elements. Composite secret
  placeholders are rejected so secrets cannot be hidden inside an otherwise inspectable string.
- Redacted launch snapshots replace secret argv values with `<secret>`.
- Structured errors do not include full argv, credential-bearing URLs, access tokens, or arbitrary
  full environments.
- The fake-Java integration fixture asserts that the access token reaches the child argv while not
  appearing in plan/game debug output or redacted snapshots.

## Internal Boundary Review

The Phase 1 crate roots are thin public facades. Mojang DTOs remain under the private `mojang`
namespace; ZIP/DEFLATE byte parsing is centralized in the hidden `graphene-core::archive` workspace primitive while native path/extraction policy remains private to `graphene-install`; launch
secret materialization remains private to the process boundary; and Java discovery/probe helpers are
kept behind the `graphene-java` public model/selection API. The modularization did not add crates or
new third-party dependencies.

## Residual / Unverified Acceptance Work

The implementation is not security-sign-off complete until the repository's required Rust commands
run successfully on Linux, Windows, and macOS and the documented real Vanilla smoke procedure is
executed. This review does not substitute source inspection for those runtime gates. Fuzz/property
campaign execution is also not recorded in this environment; bounded parser regression tests are
present, but sustained fuzzing should be run in normal development infrastructure.

## Verification Record for This Worktree

The final local verification attempt produced the following results:

- `cargo fmt --all -- --check` — **NOT RUN TO COMPLETION**: `cargo` is not installed in the
  execution environment (exit 127).
- `cargo check --workspace --all-targets` — **NOT RUN TO COMPLETION**: `cargo` is not installed (
  exit 127).
- `cargo test --workspace` — **NOT RUN TO COMPLETION**: `cargo` is not installed (exit 127).
- `cargo clippy --workspace --all-targets -- -D warnings` — **NOT RUN TO COMPLETION**: `cargo` is
  not installed (exit 127).
- `cargo doc --workspace --no-deps` — **NOT RUN TO COMPLETION**: `cargo` is not installed (exit
  127).
- `python scripts/check_architecture.py` — **PASS**.
- `cargo test --test phase1_vanilla` — **NOT RUN TO COMPLETION**: `cargo` is not installed (exit
  127).
- `cargo test --test architecture` — **NOT RUN TO COMPLETION**: `cargo` is not installed (exit 127).

The Rust toolchain could not be installed in this sandbox because executable toolchain artifacts
could not be retrieved into the container. Consequently, this review does not claim compilation,
format, Clippy, Rust test, Rustdoc, cross-platform CI, or real Vanilla smoke-test completion.
