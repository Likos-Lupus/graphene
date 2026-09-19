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
existing artifact boundary, publication stays transactional, and managed pack files converge on
the existing desired-state lockfile so launch/verify/repair/content operations see ordinary
Graphene instances.

Implemented: the `graphene-modpack` bounded context with strict bounded archive indexing and
deterministic format detection; Modrinth v1, CurseForge, Prism/MultiMC, Graphene pack v1, and
explicitly-configured generic adapters (format DTOs private); immutable content-addressed pack
source snapshots from local files or HTTPS URLs; exact CurseForge `(project_id, file_id)`
resolution through the existing `ContentProvider` boundary with persisted provenance; a
fingerprinted, validated, mutation-free `ModpackImportPlan`; generic `graphene-install` seed
layers executing base runtime, managed files, embedded mods, and ordered seeds as one staged
transaction behind one create-only publication; lockfile schema 3 with a bounded optional
`pack_origin`, schema 1/2 read compatibility, and origin-preserving mutations and repair; public
`ModpackService` inspect/plan_import/execute_import/plan_export/execute_export operations with
progress, cancellation, typed errors, and diagnostics; strict Graphene pack v1 import; and
deterministic export with explicit embedding/redistribution policy, stale-state checks,
self-validation against its own import contract, and create-only output publication. Offline
deterministic coverage includes hostile-input matrices per format, fake-provider exact-resolution
tests, transaction failure injection via shared machinery, MultiMC embedded-mod promotion, export
round-trip integration tests, and root facade workflows.

**Exit:** hosts can inspect, plan, install, verify/repair, and re-export supported packs entirely
through stable Graphene operations without provider/UI/string-parsing logic; committed instances
are indistinguishable to downstream lifecycle code from directly assembled instances; real-world
interop smokes are recorded honestly as NOT RUN in `docs/VALIDATION.md`.

## Phase 7 — Diagnostics

Implemented: the `graphene-diagnostics` bounded context and `DiagnosticService` turn existing
verification, Java, content-inventory, and launch evidence into one bounded, offline-first, read-only
diagnostic workflow.

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

Phase 8 proves the architectural claim made since the first Graphene design: the launcher engine is
actually usable from materially different hosts without forking or moving backend logic into those
hosts. The phase delivers a reference CLI and a concrete Tauri reference host. The Tauri frontend is
an integration/proof shell, not a promise to build a polished end-user launcher UI in this phase.

The most important Phase 8 result is not visual UI. It is evidence that the same root `graphene`
facade, operation model, account flow, install/content/modpack/diagnostic services, launch lifecycle,
and structured errors can be consumed by both a terminal program and an IPC/event-driven desktop
host while all Minecraft/provider/business logic stays in the engine.

### Phase 8 hard architectural decisions

Host code is an outward consumer layer:

```text
                         +--------------------+
                         |  graphene-cli      |
                         +---------+----------+
                                   |
                                   v
                     optional reference-host support
                                   |
                                   v
                              graphene
                           root public facade
                                   ^
                                   |
                     optional reference-host support
                                   |
                         +---------+----------+
                         |  graphene-tauri    |
                         |  Tauri IPC + UI    |
                         +--------------------+
```

Normative rules:

1. Engine crates never depend on `apps/`, CLI libraries, Tauri, Slint, web frontend packages, or host
   wire DTOs.
2. Reference hosts depend on the root `graphene` facade, not directly on private bounded-context
   crates. A host-specific support package may also depend on `graphene`, but may not become an
   alternate public engine facade.
3. CLI/Tauri command handlers contain input validation, presentation mapping, operation-handle
   ownership, and host lifecycle glue only. Minecraft metadata rules, provider logic, install
   algorithms, repair, content dependency logic, modpack parsing, diagnostics, and launch planning
   remain in Graphene.
4. Host wire/JSON DTOs are app-local compatibility contracts. They are not added to
   `graphene-core`, and the engine is not forced to derive serialization traits merely because a
   Tauri frontend wants a particular JSON shape.
5. Tauri frontend JavaScript/TypeScript never receives refresh/access tokens or a `LaunchSession`.
   Authentication/session refresh stays in the Rust backend. The only authentication interaction
   crossing to UI is the minimum typed user interaction intentionally meant for display (for
   example verification URI + device code), with explicit sensitive handling.
6. Hosts do not call Mojang, Microsoft, Modrinth, CurseForge, Java-distribution, or loader APIs
   directly. They configure Graphene providers and call Graphene services.
7. Hosts do not execute Java/Minecraft through shell strings. Launch remains the existing Graphene
   direct-argv lifecycle.
8. A UI request never gets authority to supply arbitrary absolute backend mutation paths. Import/
   export paths chosen by the host are passed through the existing Graphene validation/transaction
   boundaries.
9. Engine API changes discovered while implementing hosts require evidence that the gap is
   host-neutral. Do not add Tauri-specific callbacks/types to Graphene. Prefer app-local adapters;
   if both CLI and Tauri expose the same genuine engine ergonomics gap, document the generic change
   and keep it Graphene-owned.
10. Phase 8 does not add telemetry, advertisements, self-update, news feeds, dynamic plugins, a
    privileged daemon, or a remote launcher REST service.

### Phase 8 repository layout

The recommended layout is:

```text
apps/
  graphene-reference-host-support/   # optional shared host-only support
  graphene-cli/
  graphene-tauri/
    src-tauri/
    <minimal frontend sources>
```

The exact workspace arrangement may be a single Cargo workspace or a clearly separated host
workspace if Tauri/system dependency isolation materially improves CI. Whichever layout is chosen
must be recorded in `ARCHITECTURE.md` and mechanically checked. The important invariant is the
**dependency direction**, not directory aesthetics.

If a shared reference-host-support crate is introduced, its allowed responsibilities are narrow:

- explicit host data-root/config resolution;
- a production-oriented OS secure credential store adapter implementing Graphene's existing
  `SecretStore` trait;
- safe tracing initialization owned by the executable host;
- small reusable operation/event bridging helpers that depend only on public `graphene` API;
- no provider protocol logic, no instance mutation logic, no duplicate engine domain model.

### Phase 8 architecture checker evolution

The current checker treats every known package as an engine package and forbids UI dependencies.
Phase 8 must make this distinction explicit instead of weakening the rule globally:

```text
ENGINE_PACKAGES
  - graphene
  - graphene-core
  - ...
  - graphene-diagnostics
  - graphene-service

HOST_PACKAGES
  - graphene-reference-host-support (if present)
  - graphene-cli
  - graphene-tauri-host
```

Checks must enforce:

- UI dependencies are still forbidden in every engine package;
- no engine package depends on a host package;
- host packages use root `graphene` rather than reaching around it into internal crates, except for
  a specifically documented build-only necessity approved by architecture review;
- provider DTOs and Reqwest do not migrate into hosts as a shortcut;
- the complete dependency graph remains acyclic;
- engine crate-root thinness/hygiene remains unchanged;
- host source-size/hygiene rules are also bounded so `main.rs`/Tauri command files do not become new
  god files.

### Phase 8 reference host configuration

Both hosts must construct Graphene explicitly and share the same conceptual configuration model:

```text
host-owned data-root selection
        +
network/proxy/retry policy
        +
provider endpoint/key configuration
        +
optional Microsoft application configuration
        +
secure SecretStore implementation
        |
        v
GrapheneBuilder::build()
```

The engine still requires an explicit data root. Hosts may choose platform-appropriate defaults,
command-line overrides, or user settings, but the selected path is resolved before constructing
Graphene and is never a hidden process-global engine singleton.

Provider credentials and Microsoft application configuration must not be committed in repository
config or printed by debug output. Environment variables/config files may identify non-secret
settings; actual secrets use secret-aware host handling.

### Phase 8 production secure-store reference adapter

Phase 2 intentionally left production secret-vault selection to the distributor/host. Phase 8 is
the right place to provide a **reference host implementation**, not to move OS credential APIs into
`graphene-auth`.

The reference host should implement `SecretStore` using the current supported platform credential
vault/keyring abstraction selected at implementation time. OpenCode must verify the current crate
API and platform behavior through Context7/official documentation rather than assume an old keyring
API.

Requirements:

- Windows/macOS/Linux platform behavior is explicit and documented;
- no plaintext fallback if the secure backend is unavailable;
- stable Graphene namespacing uses `SecretRecordIdentity::key()` or an equivalent public identity;
- secret values are never included in `Debug`, logs, CLI JSON, Tauri events, panic messages, or
  persisted host config;
- blocking OS-vault calls are not performed on an async I/O worker if the selected implementation
  can block;
- automated tests use a deterministic fake/in-memory vault, not the developer's real credential
  store;
- real keyring smoke status is recorded honestly in `VALIDATION.md` per supported OS.

Offline-account-only workflows must continue to work when no secure vault is available. Microsoft
persistent login must fail explicitly rather than silently degrade to plaintext.

### Phase 8 operation bridge contract

Both hosts need to manage long-running Graphene operations without inventing a new engine task
system. Host-side glue should normalize the common pattern:

```text
start Graphene operation
        |
        +--> retain OperationHandle by OperationId
        |
        +--> subscribe/forward bounded OperationEvent snapshots
        |
        +--> await typed result in host-owned async task
        |
        +--> expose explicit cancel(operation_id)
        |
        +--> remove terminal host registry entry after a bounded retention policy
```

The registry is host state, not a global Graphene singleton. It must handle:

- operation IDs that are unknown/already terminal;
- client disconnect/window close;
- cancellation race at Graphene's documented point of no return;
- bounded event forwarding/backpressure;
- terminal result/error retrieval;
- no secret-bearing result serialization where the engine intentionally keeps a value backend-only.

Do not create one ad hoc progress/cancel mechanism per CLI command or Tauri command.

### Phase 8 running-game bridge

`RunningGame` ownership is similarly host-side. A host registry may retain a running game handle and
map it to a safe opaque game/run ID. It must preserve the engine's instance shared lease until the
process really terminates.

Hosts may expose:

```text
run id / instance id
pid (where safe/useful)
bounded GameEvent stream
wait/terminal result
kill request
dropped-output count
```

but they must not expose Tokio child handles or bypass `RunningGame::kill()/wait()`.

For Tauri, access/session secrets remain inside Rust and must not be serialized to the webview. A
frontend "launch" request should identify an account/instance; the Rust host obtains the
`LaunchSession`, builds a `LaunchPlan`, and executes it entirely backend-side.

### Phase 8 reference CLI

The CLI is a real consumer of the root facade, not a thin demo that only prints a version. It should
cover every major service family with at least one useful end-to-end path.

Recommended command surface:

```text
graphene-cli engine info

graphene-cli instance list|get|verify|repair|clone|delete|config ...
graphene-cli install vanilla ...
graphene-cli install loader ...

graphene-cli account list|offline-add|microsoft-login|reauth|remove ...
graphene-cli java list|ensure|install ...

graphene-cli content scan|search|recognize|install|update ...
graphene-cli modpack inspect|import|export ...

graphene-cli launch plan|run|kill ...
graphene-cli diagnose ...
```

Exact subcommand spelling is a host concern, but command implementation must demonstrate the
corresponding existing engine capabilities.

The CLI must support two presentation modes:

1. a human-readable terminal mode with progress and safe concise diagnostics;
2. a machine-readable `--json` (or equivalent) mode with documented stable host DTOs and no ANSI
   control sequences.

CLI rules:

- machine mode never requires parsing human prose to discover `ErrorCode`, operation state, finding
  code, recommendation, or IDs;
- exit-code mapping is documented and coarse-grained; Graphene's detailed `ErrorCode` remains in
  structured output;
- `Ctrl+C` requests cancellation of the active cancellable Graphene operation. A process-launch
  command has a separately documented graceful/kill policy and must not abandon the `RunningGame`
  lease silently;
- Microsoft device interaction may intentionally print the verification URI/user code to the
  interactive terminal, but those values are not sent to tracing/log files and are handled as
  transient sensitive interaction data;
- secrets supplied by environment/config are never echoed by `--verbose`;
- launch-plan display uses the existing redacted snapshot rather than exposing secret arguments;
- commands that mutate an existing instance use the existing plan/lease/transaction APIs and show
  inspectable plans where the engine provides them.

### Phase 8 Tauri reference host

The Tauri deliverable proves IPC/event-driven consumption without contaminating the engine. It may
use a deliberately minimal frontend. The acceptance target is architecture and lifecycle behavior,
not production visual design.

Rust-side app state should contain only host composition/registries, conceptually:

```text
TauriAppState
  - Graphene
  - host operation registry
  - running-game registry
  - host configuration snapshot
  - host event bridge
```

Tauri commands should be grouped by domain rather than accumulated into one giant command file:

```text
commands/engine
commands/instances
commands/install
commands/accounts
commands/java
commands/content
commands/modpacks
commands/launch
commands/diagnostics
commands/operations
```

App-local wire DTOs map to/from Graphene-owned API types. The mapping layer must:

- expose stable string forms for IDs/codes/enums intentionally chosen by the host;
- omit internal source errors, absolute secret paths, and implementation handles;
- serialize redacted launch plans only;
- never serialize refresh/access credentials or `LaunchSession`;
- bound large log/output payloads and event rates;
- return explicit error envelopes containing safe code/kind/context rather than throwing arbitrary
  Rust debug strings at the frontend.

Tauri event forwarding should use a small number of typed channels/envelopes, for example operation,
game, and authentication-interaction events, instead of generating a unique event name for every
backend action. The frontend should be able to correlate events by operation/run ID.

The minimal frontend must demonstrate at least:

- engine/data-root status;
- instance inventory;
- start/cancel/observe one long operation;
- account interaction path (offline always; Microsoft when configured);
- install or modpack import plan/execution path using fixtures or real configuration as appropriate;
- launch plan/run lifecycle with bounded output;
- diagnostic report presentation using codes/severity/confidence rather than message parsing.

Playwright/browser tooling may be used for deterministic frontend behavior where applicable, but it
is not a substitute for Rust command/engine integration tests.

### Phase 8 host API safety and serialization boundary

Phase 8 is expected to reveal which root-facade values are ergonomic. The response to friction must
be disciplined:

- do not publicly expose Reqwest, Tokio child/channel, filesystem lease, provider DTO, Tauri, or
  keyring types from `graphene`;
- do not make every opaque engine handle `Serialize` just for IPC;
- do not add stringly "execute(action, json)" catch-all APIs to avoid typed adapters;
- do not turn Graphene into a global singleton because Tauri state is global to one app;
- do not expose arbitrary shell/prelaunch execution as a convenience host escape hatch;
- prefer host-local DTO conversion and registries;
- if a generic engine API is genuinely missing, add the smallest Graphene-owned typed capability
  and prove it is useful from both CLI and Tauri.

### Phase 8 reference tracing/logging behavior

Executables, not the engine, own tracing subscribers and output destinations. Reference hosts should
initialize structured tracing with safe defaults and allow user-controlled verbosity without
turning secrets on.

Requirements:

- default logs contain correlation IDs, operation stage, safe provider/service names, and error
  codes where useful;
- no bearer/refresh tokens, Microsoft user/device codes, CurseForge keys, proxy credentials, or
  secret launch arguments;
- Tauri frontend console logs must not receive Rust secret values;
- diagnostic excerpts are already redacted by Phase 7 before a host chooses to display/export them;
- file logging, if provided, is host-owned and has bounded rotation/retention rather than
  unbounded append.

### Phase 8 test strategy

The host phase needs its own deterministic tests while preserving the engine's offline normal CI.

#### CLI tests

- parser/subcommand validation;
- human vs JSON output separation;
- stable safe error envelope/exit-code mapping;
- progress/event rendering from synthetic operations;
- cancellation on simulated interrupt;
- redacted launch-plan output;
- no secret sentinels in verbose/JSON/stderr;
- end-to-end fixture workflow using an isolated temporary data root and fake/local providers where
  existing Graphene fixtures allow it;
- fake Java launch lifecycle, wait, kill, and dropped-output behavior;
- diagnose command consumes the Phase 7 report without string matching.

#### Tauri Rust adapter tests

- command input validation and DTO conversion;
- operation registry insert/event/terminal/cancel/removal behavior;
- disconnected listener / bounded-event behavior;
- running-game registry lifecycle and lease-safe cleanup;
- authentication interaction serialization contains only intended fields;
- session/refresh/access secret types are impossible to serialize through normal command results;
- error envelope contains stable safe Graphene error information;
- each major command group calls root facade services rather than provider/network shortcuts.

#### Tauri frontend tests

- deterministic rendering of instance/operation/diagnostic states using mocked command/event
  fixtures;
- cancellation and terminal-state UI behavior;
- no dependence on parsing human diagnostic/error text;
- reconnect/reload does not fabricate backend operation success.

#### Cross-host contract tests

A shared deterministic scenario should be exercised through both host adapters and compared at the
Graphene result level, for example:

```text
same fixture data root input
  -> list/verify an instance
  -> obtain structured diagnostic report
  -> plan a safe action
```

The human/UI presentation may differ; the underlying Graphene IDs/codes/state/outcome must agree.
This is the core proof that backend logic was not forked into the host.

### Phase 8 real smoke and validation policy

`VALIDATION.md` must add separate real-host procedures. Suggested procedures include:

1. **CLI Vanilla/loader lifecycle smoke** - build the real CLI, use production provider defaults,
   install a pinned supported runtime, obtain a valid session as applicable, launch, observe, and
   terminate.
2. **CLI Microsoft + secure vault smoke** - on a supported desktop OS, use the real reference secure
   store and verify restart/refresh/remove without credential leakage.
3. **Tauri adapter smoke** - build/package the reference desktop host, execute at least instance
   inventory, one cancellable operation, launch lifecycle, and diagnostics through IPC/events.
4. **Host parity smoke** - point CLI and Tauri at separate equivalent test roots and prove the same
   engine operations produce semantically equivalent normalized outcomes.

These remain `NOT RUN` until actually executed. A successful Tauri command unit test is not a real
packaged-desktop smoke; a fake Java process is not a real Minecraft launch.

### Phase 8 implementation workstreams (OpenCode execution order)

#### 8.0 — Baseline and host-boundary audit

- Re-read all canonical docs plus `SKILLS.md` and any execution-time `AGENTS.md`.
- Use RustRover MCP to map every `Graphene` service accessor, operation wrapper, account
  interaction, `RunningGame`, diagnostics, and root re-export needed by hosts.
- Re-run engine quality gates before adding host dependencies.
- Decide the Cargo/workspace placement for reference apps and record the reason before code changes.
- Verify current Tauri, CLI argument parser, OS keyring, serialization, and frontend build APIs with
  Context7/official docs; do not code against remembered versions.

**Gate 8.0 exit:** the host architecture can be added without weakening the engine's UI-dependency
ban or reaching into private crates.

#### 8.1 — Architecture/hygiene support for host packages

- Teach architecture checks to distinguish engine and host packages.
- Keep all old engine prohibitions intact.
- Add host dependency-direction and source-size checks.
- Establish modular app directory skeletons; `main.rs`/Tauri command root must remain thin.

**Gate 8.1 exit:** adding `tauri` to the designated host is allowed while adding it to any engine
crate still fails the checker.

#### 8.2 — Reference host bootstrap/configuration

- Implement explicit data-root resolution, safe config loading, provider configuration mapping, and
  tracing initialization.
- Keep secrets out of persisted ordinary config and debug output.
- Prove two host processes/data roots do not rely on a process-global Graphene singleton.

**Gate 8.2 exit:** both host executables can construct the same root `Graphene` facade from explicit
host configuration with deterministic test overrides.

#### 8.3 — Reference secure credential store

- Implement the host-owned OS-vault `SecretStore` adapter.
- Use blocking boundaries where required by the chosen OS-vault library.
- Add deterministic fake-store tests and explicit unavailable-backend behavior.
- Add manual per-OS real smoke procedures to `VALIDATION.md`.

**Gate 8.3 exit:** persistent Microsoft auth can be wired by a real host without any plaintext
fallback entering engine storage.

#### 8.4 — Shared host operation/run bridging primitives

- Implement bounded operation registry/event forwarding/cancellation.
- Implement running-game handle registry and lifecycle cleanup.
- Keep these abstractions host-local and based only on root Graphene types.

**Gate 8.4 exit:** synthetic operations and fake Java runs can be observed/cancelled/waited through
host registries without leaking Tokio/process internals or losing terminal state.

#### 8.5 — Reference CLI foundation

- Add modular command parser, config/bootstrap, output abstraction, safe error envelope, and
  human/JSON modes.
- Add Ctrl+C operation cancellation and tracing behavior.
- Keep command modules thin orchestrators over Graphene services.

**Gate 8.5 exit:** CLI engine/info, operation observation, safe errors, and JSON output pass tests
before broad service commands are added.

#### 8.6 — CLI instance/install/account/Java workflows

- Wire instance inventory/verify/repair/lifecycle commands.
- Wire Vanilla/loader install planning/execution.
- Wire offline account and Microsoft interaction/session behavior.
- Wire Java inventory/ensure/install.
- Use inspectable/redacted plan output where appropriate.

**Gate 8.6 exit:** the CLI can exercise the launcher backbone without any direct provider protocol or
filesystem mutation logic.

#### 8.7 — CLI content/modpack/launch/diagnostics workflows

- Wire offline content scan and representative provider-backed content operations.
- Wire modpack inspect/plan/import/export.
- Wire launch plan/run/wait/kill with redacted plan output.
- Wire Phase 7 diagnosis and structured recommendation display.

**Gate 8.7 exit:** every major Graphene service family has a real reference CLI consumption path and
machine output does not parse human messages.

#### 8.8 — Tauri Rust adapter foundation

- Add modular app state, command registration, safe wire DTOs, error envelopes, and event bridge.
- Add operation/run registries to managed Tauri state.
- Ensure `LaunchSession` and credential values cannot cross IPC accidentally.

**Gate 8.8 exit:** Tauri command tests can call engine status/instance inventory and observe a
synthetic cancellable operation through typed events.

#### 8.9 — Tauri service command groups

- Add domain-grouped commands for accounts, install, instance, Java, content, modpack, launch, and
  diagnostics.
- Keep complex algorithms in Graphene; command modules only validate/convert/orchestrate.
- Add backend tests for every command group and secret sentinel tests.

**Gate 8.9 exit:** command coverage demonstrates the same engine capabilities as the CLI without
provider/network/business duplication.

#### 8.10 — Minimal frontend integration shell

- Build only enough UI to exercise typed command/event flows and lifecycle states.
- Present operation progress, authentication interaction, game output/lifecycle, and diagnostic
  codes/severity/confidence.
- Do not implement a full launcher design system, updater, telemetry, news, or plugin marketplace.
- Use Playwright/browser-oriented tests where they are deterministic and suitable; keep Rust adapter
  tests authoritative for backend behavior.

**Gate 8.10 exit:** a user can drive the defined proof workflows through the Tauri shell without
frontend string parsing or secret/session leakage.

#### 8.11 — Cross-host parity and architecture regression suite

- Run shared scenarios through CLI and Tauri adapters.
- Add mechanical checks that hosts import root `graphene` rather than internal implementation
  crates.
- Search for accidental direct HTTP/provider calls, duplicated Minecraft/install/repair logic,
  shell execution, and plaintext secret persistence in app code.

**Gate 8.11 exit:** different host presentation layers produce equivalent Graphene-level outcomes
and the architecture checker proves engine-to-host dependency reversal is absent.

#### 8.12 — Packaging, shutdown, and failure behavior

- Define graceful application shutdown: cancel/retain operations deliberately, terminate or keep
  games according to explicit user action, and never silently drop a runtime lease while the child
  is alive.
- Test window close/reload, CLI interrupt, failed secure store, provider unavailable, partial host
  configuration, and Tauri event-consumer disconnect.
- Keep packaging/signing/distribution credentials outside the repository.

**Gate 8.12 exit:** host shutdown/error paths preserve Graphene state/lease/transaction guarantees
and do not leave a valid-looking half-completed operation.

#### 8.13 — Canonical docs and validation convergence

Update:

- `API.md` only for any genuinely new generic Graphene API discovered by host integration;
- `ARCHITECTURE.md` with host package dependency direction and engine/host checker distinction;
- `SECURITY.md` with OS-vault and Tauri IPC/trust boundaries;
- `VALIDATION.md` with CLI/Tauri/secure-store/host-parity real smoke procedures and actual status;
- `README.md` with reference-host entry points without redefining engine architecture;
- this roadmap, collapsing Phase 8 to an implemented summary only after acceptance.

Do not create a permanent second host-protocol specification masquerading as Graphene's public API.

**Gate 8.13 exit:** canonical documents cleanly distinguish engine contracts from reference-host
contracts and all smoke evidence is honest.

#### 8.14 — Final release-quality gate

Run the full engine quality suite plus all host-specific builds/tests. At minimum:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
python scripts/check_architecture.py
python scripts/check_hygiene.py
```

If hosts live in a separate Cargo workspace, run equivalent `fmt/check/test/clippy/doc` commands
against that workspace as well. Run the frontend package's deterministic format/typecheck/test/build
gates required by the selected Tauri stack. Use RustRover build/problems as an additional mandatory
feedback loop, not a replacement for the Cargo gates.

**Phase 8 exit:** a reference CLI and a Tauri reference host both consume the same root Graphene
facade for meaningful launcher workflows; operations/cancellation/errors/diagnostics/game lifecycle
cross both host boundaries without business-logic forks; persistent Microsoft credentials can use a
host-supplied production secure vault without plaintext fallback; UI/IPC DTOs and dependencies stay
outside the engine; all automated engine and host gates pass; and any real provider/Minecraft/OS
vault/packaged-host smokes are recorded as PASS only when actually executed.

### OpenCode execution protocol for Phases 7 and 8

These roadmap sections are intentionally detailed enough to serve as the implementation sequence.
When OpenCode executes either phase:

1. Treat the canonical repository docs as architecture authority and this roadmap as chronology/
   planned behavior. Do not rewrite architecture to match an easier implementation.
2. Treat `SKILLS.md` and any `AGENTS.md` present in the implementation baseline as mandatory tooling
   and engineering protocol.
3. Use `todowrite` for the multi-gate implementation and keep one active gate at a time.
4. Use RustRover MCP as the first choice for project tree exploration, symbol lookup, call hierarchy,
   renaming, semantic edits, problems/lints, build, formatting, and targeted test/run operations as
   required by `SKILLS.md`.
5. Use Context7/official documentation for current external crate/framework APIs and GitHub code
   search only as reference; external examples never override Graphene's dependency/security
   boundaries.
6. Use subagents for separable audits/tests only when their scope is explicit. The primary agent
   remains responsible for integrating results and re-running the real repository gates.
7. Keep crate roots and host entry points thin from the first change. Never temporarily accept a
   1,000-line `lib.rs`, `main.rs`, or Tauri command god file on the promise of splitting it later.
8. Add deterministic offline tests with each gate rather than implementing all code first and tests
   at the end.
9. Never report a command, real provider smoke, real Minecraft launch, secure-vault test, or packaged
   Tauri smoke as PASS unless it was actually executed successfully in the current implementation
   environment.
10. After a phase is implemented, move durable behavior into `API.md`/`ARCHITECTURE.md`/
    `SECURITY.md`/`VALIDATION.md` and collapse the detailed phase block in this roadmap to a concise
    implemented summary, preserving `ROADMAP.md` as chronology rather than a permanent duplicate
    specification.

## Priority policy

P0 work preserves the install-to-launch backbone and safety boundaries. P1 completes a modern
launcher backend. P2 is ecosystem breadth/convenience and must not destabilize P0/P1 contracts.
