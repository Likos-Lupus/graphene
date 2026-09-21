# Graphene Implementation Roadmap

This is the canonical home for development chronology. Current behavior belongs in `API.md`, trust
boundaries in `SECURITY.md`, and validation status in `VALIDATION.md`.

## Phase 0 — Foundation

Workspace/facade, core operation/error/artifact types, platform/network/storage boundaries, verified
artifact acquisition, cancellation/progress, and deterministic local-fixture testing.

**Exit:** cross-platform build/test gates, verified download/cache behavior, safe cancellation, and
no UI dependency.

## Phase 1 — Vanilla end-to-end

Mojang metadata normalization, resolved Minecraft, deterministic create-only installation, local
Java selection, offline launch planning, and direct process lifecycle.

**Exit:** from an empty data root, install and launch a pinned Vanilla version through the library,
including restart/offline planning and immutable reuse evidence.

## Phase 2 — Authentication and managed Java

Provider-neutral accounts, Microsoft device/refresh chain, injected secure credential storage,
offline identities, managed Java resolution/staged installation, and Java diagnostics.

**Repository status:** implementation candidate. Real authentication/managed-Java smoke remains
`NOT RUN`, a production secure vault is host/distributor supplied, and Rust quality gates cannot run
in the current toolchain-less environment.

## Phase 3 — Loader components

Component graph/patch model, provider registry, Fabric/Forge/NeoForge adapters, Forge-family staged
Java preparation, generated-output verification/reuse, and receipt component persistence.

**Repository status:** implementation candidate. Deterministic fixture coverage exists; real loader
smokes and Rust quality gates remain pending. See `LOADER_SUPPORT.md` and `VALIDATION.md`.

## Phase 4 — Instance engine

Centralized committed-state repository, resilient inventory discovery, global defaults and
per-instance configuration hierarchy with explicit tri-state patch semantics, cross-process advisory
shared/exclusive leases (`fs2`) backed by persistent carriers, schema-versioned provider-neutral
desired-state lockfile, runtime lease retention with stale-plan protection, atomic rename, streamed
containment-safe clone, quarantine delete transactions, quick and full verification, and
deterministic repair planning and execution reusing install primitives.

**Exit:** repair is a deterministic planned transaction derived from durable desired state plus a
filesystem diff, mutable instance operations share one repository/lease/transaction model, and no
Phase 4 capability depends on ad hoc mutation or provider-specific persisted state.

## Phase 5 — Content

Provider-neutral content bounded context (`graphene-content`), offline mod scanning
(`.minecraft/mods`), bounded metadata inspection (Fabric `fabric.mod.json`, Forge
`META-INF/mods.toml`, NeoForge `META-INF/neoforge.mods.toml`, Legacy `mcmod.info`), streaming SHA-1
and SHA-256 local identities, CurseForge-compatible Murmur2 lookup fingerprinting, Modrinth and
optional CurseForge providers with strict secret redaction, deterministic bounded
required-dependency resolution, inspectable non-mutating `ContentMutationPlan`, journaled
crash-recoverable execution under exclusive instance leases, schema-2 `lock.json` desired-state
evolution with schema-1 read compatibility, and convergence with existing instance verification and
provider-neutral repair.

**Exit:** hosts have provider-neutral local mod inventory, discovery, recognition, exact
install/update and dependency/compatibility planning; all managed content mutations use one
inspectable stale-protected transaction model and persist into the existing desired-state lockfile;
verified artifact acquisition, instance verification, repair, clone/delete locking, cancellation,
security, and provider isolation remain convergent rather than forming a second launcher pipeline.

## Phase 6 — Modpacks

External modpack/container formats are normalized into one Graphene-owned, deterministic,
create-only installation model without introducing a second launcher pipeline: pack input is
untrusted, runtime identities resolve through the existing component stack, downloads use the
existing artifact boundary, publication stays transactional, and managed pack files converge on the
existing desired-state lockfile so launch/verify/repair/content operations see ordinary Graphene
instances.

Implemented: the `graphene-modpack` bounded context with strict bounded archive indexing and
deterministic format detection; Modrinth v1, CurseForge, Prism/MultiMC, Graphene pack v1, and
explicitly-configured generic adapters (format DTOs private); immutable content-addressed pack
source snapshots from local files or HTTPS URLs; exact CurseForge `(project_id, file_id)`
resolution through the existing `ContentProvider` boundary with persisted provenance; a
fingerprinted, validated, mutation-free `ModpackImportPlan`; generic `graphene-install` seed layers
executing base runtime, managed files, embedded mods, and ordered seeds as one staged transaction
behind one create-only publication; lockfile schema 3 with a bounded optional
`pack_origin`, schema 1/2 read compatibility, and origin-preserving mutations and repair; public
`ModpackService` inspect/plan_import/execute_import/plan_export/execute_export operations with
progress, cancellation, typed errors, and diagnostics; strict Graphene pack v1 import; and
deterministic export with explicit embedding/redistribution policy, stale-state checks,
self-validation against its own import contract, and create-only output publication. Offline
deterministic coverage includes hostile-input matrices per format, fake-provider exact-resolution
tests, transaction failure injection via shared machinery, MultiMC embedded-mod promotion, export
round-trip integration tests, and root facade workflows.

**Exit:** hosts can inspect, plan, install, verify/repair, and re-export supported packs entirely
through stable Graphene operations without provider/UI/string-parsing logic; committed instances are
indistinguishable to downstream lifecycle code from directly assembled instances; real-world interop
smokes are recorded honestly as NOT RUN in `docs/VALIDATION.md`.

## Phase 7 — Diagnostics

Implemented: the `graphene-diagnostics` bounded context and `DiagnosticService` turn existing
verification, Java, content-inventory, and launch evidence into one bounded, offline-first,
read-only diagnostic workflow.

- `graphene-diagnostics` owns provider-neutral request/report/finding/evidence/recommendation models
  with hard bounds and confidence ordering, bounded lossy text and line scanning, deterministic
  fail-closed secret/path redaction, a substring (non-regex) crash/log rule set, and instance/Java/
  content correlation with deduplicated, deterministically ordered, non-mutating recommendations. It
  depends only on `graphene-core`, `graphene-instance`, and `graphene-content`.
- `DiagnosticService` (`Graphene::diagnostics().analyze(...)`) orchestrates collection under the
  shared instance lease, reuses existing verification/content scan/Java selection, collects an
  allowlisted bounded set of local sources, compares before/after state and content fingerprints,
  and returns a `DiagnosticReport` through the existing operation/cancellation model with stages
  `snapshot`, `verify`, `java`, `collect_logs`, `scan_content`, `analyze`, `redact`, `complete`.
- Diagnosis is read-only. Recommendations (`PlanInstanceRepair`, `SelectCompatibleJava`,
  `InstallManagedJava`, `ReviewContentConflict`, `ReviewMissingContentDependency`,
  `ReviewMemoryConfiguration`, `ReviewGraphicsOrNativeEnvironment`, `CollectAdditionalEvidence`,
  `NoAutomaticRemediation`, and others) point at existing services and require explicit invocation;
  there is no hidden mutation and no second repair pipeline. Unknown crashes resolve to
  `DIAGNOSTIC_CAUSE_UNDETERMINED` rather than a fabricated blame assignment.
- `scripts/check_architecture.py` treats `graphene-diagnostics` as an engine crate and enforces its
  dependency direction plus reverse-edge prohibitions. Canonical behavior lives in `API.md`,
  `ARCHITECTURE.md`, `SECURITY.md`, and `VALIDATION.md`.

**Exit:** hosts can request a bounded, offline-first, secret-redacted `DiagnosticReport` correlating
instance verification, Java evidence, local content state, known crash/log patterns, and optional
process-exit evidence; every finding has typed severity/confidence/evidence, every recommendation is
non-mutating and converges on an existing Graphene service, unknown causes remain explicit, no
UI/provider DTO leaks into the domain, and all automated quality gates pass. Real Minecraft/provider
diagnostic smokes remain `NOT RUN` per `VALIDATION.md`.

## Phase 8 — Host integration

Implemented: `apps/` is a separate host workspace containing a shared reference-host-support crate,
a reference CLI, and a Tauri reference host, all consuming only the root `graphene` facade.

- **Layout.** `apps/Cargo.toml` is its own workspace so Tauri/graphics system dependencies never
  enter the engine workspace gates or its cross-platform CI matrix.
  `graphene-reference-host-support` owns explicit data-root/config resolution, a production OS
  credential-vault `SecretStore` adapter, tracing initialization, and bounded operation/run bridging
  registries. `graphene-cli` and `graphene-tauri` (`src-tauri`) are outward consumers; a minimal
  Vite/TypeScript shell under `graphene-tauri` exercises typed commands and events.
- **Checker split.** `scripts/check_architecture.py` distinguishes `ENGINE_PACKAGES` from
  `HOST_PACKAGES`: engine UI-dependency bans stay absolute; no engine package depends on a host;
  hosts depend only on root `graphene` (plus other hosts), never on internal engine crates; Tauri is
  allowed only in `graphene-tauri-host`; Reqwest/provider namespaces and shell-execution patterns
  stay banned in hosts; the combined graph stays acyclic; host `lib.rs`/`main.rs` entry points stay
  thin.
  `scripts/check_hygiene.py` now scans host Rust and TypeScript sources for production size and
  phase-naming.
- **Secure store.** `KeyringSecretStore` implements Graphene's `SecretStore` over the platform vault
  (macOS Keychain, Windows Credential Manager, Secret Service on non-macOS `*nix`) using
  `SecretRecordIdentity::key()`, with no plaintext fallback; deterministic tests use a fake store.
- **Bridges.** `HostOperationRegistry` retains `OperationHandle`s, forwards bounded events, exposes
  cancel/terminal state, and removes entries after a retention window. `HostRunRegistry` retains
  `RunningGame` handles behind a `HostRun` abstraction (testable with fake runs), preserving the
  instance lease until the child terminates.
- **CLI.** `graphene-cli` covers
  engine/instance/install/account/Java/content/modpack/launch/diagnose with human and stable
  `--json` modes, a safe error envelope, documented coarse exit codes, Ctrl+C cancellation, and no
  secret leakage in verbose/JSON/stderr.
- **Tauri host.** `graphene-tauri-host` exposes domain-grouped commands over managed host state with
  app-local wire DTOs, safe error envelopes, and typed operation/game/auth event channels.
  Refresh/access tokens and `LaunchSession` never cross IPC; the only authentication material
  crossing is the intended device-code display pair. Window exit explicitly terminates tracked
  games.
- **API addition.** Host integration surfaced one genuine host-neutral gap: modpack request/plan/
  result types are now re-exported by the root facade for both hosts.
- Canonical behavior lives in `API.md`, `ARCHITECTURE.md`, `SECURITY.md`, `VALIDATION.md`, and
  `README.md`; no permanent second host-protocol specification was created.

**Exit:** a reference CLI and a Tauri reference host both consume the same root facade for
meaningful launcher workflows; operations/cancellation/errors/diagnostics/game lifecycle cross both
host boundaries without business-logic forks; persistent Microsoft credentials can use a
host-supplied production vault without plaintext fallback; UI/IPC DTOs and dependencies stay outside
the engine; all automated engine and host gates pass. Real provider/Minecraft/OS-vault/packaged-host
smokes remain
`NOT RUN` per `VALIDATION.md`.

### OpenCode execution protocol for Phases 7 and 8

These roadmap sections are intentionally detailed enough to serve as the implementation sequence.
When OpenCode executes either phase:

1. Treat the canonical repository docs as architecture authority and this roadmap as chronology/
   planned behavior. Do not rewrite architecture to match an easier implementation.
2. Treat `SKILLS.md` and any `AGENTS.md` present in the implementation baseline as mandatory tooling
   and engineering protocol.
3. Use `todowrite` for the multi-gate implementation and keep one active gate at a time.
4. Use RustRover MCP as the first choice for project tree exploration, symbol lookup, call
   hierarchy, renaming, semantic edits, problems/lints, build, formatting, and targeted test/run
   operations as required by `SKILLS.md`.
5. Use Context7/official documentation for current external crate/framework APIs and GitHub code
   search only as reference; external examples never override Graphene's dependency/security
   boundaries.
6. Use subagents for separable audits/tests only when their scope is explicit. The primary agent
   remains responsible for integrating results and re-running the real repository gates.
7. Keep crate roots and host entry points thin from the first change. Never temporarily accept a
   1,000-line `lib.rs`, `main.rs`, or Tauri command god file on the promise of splitting it later.
8. Add deterministic offline tests with each gate rather than implementing all code first and tests
   at the end.
9. Never report a command, real provider smoke, real Minecraft launch, secure-vault test, or
   packaged Tauri smoke as PASS unless it was actually executed successfully in the current
   implementation environment.
10. After a phase is implemented, move durable behavior into `API.md`/`ARCHITECTURE.md`/
    `SECURITY.md`/`VALIDATION.md` and collapse the detailed phase block in this roadmap to a concise
    implemented summary, preserving `ROADMAP.md` as chronology rather than a permanent duplicate
    specification.

## Priority policy

P0 work preserves the install-to-launch backbone and safety boundaries. P1 completes a modern
launcher backend. P2 is ecosystem breadth/convenience and must not destabilize P0/P1 contracts.
