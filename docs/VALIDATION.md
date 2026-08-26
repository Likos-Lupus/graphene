# Graphene Validation

This document is the canonical home for automated quality gates and manual/external smoke
procedures. Deterministic fixtures are valuable coverage, but they are not evidence that a real
upstream service or Minecraft runtime smoke passed.

## Automated quality gate

Run from the repository root:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
python scripts/check_architecture.py
python scripts/check_hygiene.py
```

### Current automated-gate status

| Gate                                                    | Status | Evidence/reason                                                   |
|---------------------------------------------------------|--------|-------------------------------------------------------------------|
| `cargo fmt --all -- --check`                            | PASS   | Clean formatting across all crates, tests, and facade.            |
| `cargo check --workspace --all-targets`                 | PASS   | Clean workspace compilation across all crates.                    |
| `cargo test --workspace`                                | PASS   | All unit, integration, and facade tests passed (202 tests total). |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS   | 0 warnings across entire workspace.                               |
| `cargo doc --workspace --no-deps`                       | PASS   | Clean rustdoc generation across workspace.                        |
| `python scripts/check_architecture.py`                  | PASS   | Strict layer boundaries and forbidden dependency checks passed.   |
| `python scripts/check_hygiene.py`                       | PASS   | Line count, facade, and hygiene rules passed.                     |

Do not convert a `NOT RUN` into PASS based on source inspection. Normal automated tests must use
local deterministic fixtures rather than public-internet availability.

## Vanilla real smoke

**Status: NOT RUN.** The prior delivery could not build/run the engine in its sandbox, and this
cleanup environment likewise has no Rust toolchain. No deterministic fixture is substituted for a
real smoke.

Procedure:

1. Start from a new empty Graphene data root with production provider defaults.
2. Select an explicit pinned supported Vanilla version; do not use a moving `latest` identity for
   acceptance.
3. Resolve official metadata and inspect the deterministic `InstallPlan` before execution.
4. Execute installation and verify the committed instance appears only after success.
5. Verify client, libraries, assets, logging metadata, natives, and immutable cache/materialization
   state.
6. Discover or provide compatible Java and inspect the normalized probe.
7. Supply an authorized ephemeral `LaunchSession` when the selected client requires it.
8. Restart/disable provider access and reconstruct a redacted `LaunchPlan` from committed state.
9. Launch through Graphene; observe bounded start/output/terminal lifecycle, then close/kill and
   confirm wait completes.
10. Install the same exact version to a second create-only instance and verify immutable artifact
    reuse.
11. Record only safe platform/version/Java/revision/instance/lifecycle evidence.

## Microsoft authentication real smoke

**Status: NOT RUN.** A real run requires a built Graphene engine, distributor-owned Microsoft app
registration/endpoints, a production secure credential backend, and a test account that owns Java
Edition. Those prerequisites are not available here.

Procedure:

1. Configure production Microsoft identity/Xbox/Minecraft endpoints without logging credentials.
2. Inject a real secure `SecretStore`; do not use an in-memory test store as production evidence.
3. Begin Microsoft login and verify the host receives a typed UI-independent device interaction.
4. Complete the official user flow and observe entitlement/profile/account-persistence stages.
5. Confirm public account metadata contains no token/device/user code or unrelated private claims.
6. Restart with the same data root and secure store, then obtain a launch session through account
   refresh without host-side OAuth/Xbox/XSTS/Minecraft protocol logic.
7. Exercise account removal and confirm public metadata plus secret credential record are removed.
8. Never record tokens/device codes; record only safe identifiers/stages/outcome.

## Managed Java real smoke

**Status: NOT RUN.** This environment cannot build Graphene or contact the configured production
Java provider.

Procedure:

1. Use an empty data root and production managed-Java provider configuration.
2. Request a supported `JavaRequirement` through `install_managed` or `ensure_for_instance`.
3. Observe release resolution, verified archive acquisition, extraction, probe, and commit stages.
4. Confirm the chosen archive uses expected integrity/HTTPS policy.
5. Confirm `shared/runtimes/<id>/runtime.json` stores a relative executable path and no absolute
   data root.
6. Confirm probe major/architecture match the requested/release identity before publication.
7. Restart and verify `managed_runtimes()` discovers/revalidates the committed runtime.
8. Repeat ensure/install and verify immutable runtime/cache reuse rather than in-place replacement.
9. Record safe release hash/runtime ID/probe/platform/outcome evidence.

## Fabric real smoke

**Status: NOT RUN.** Pinned acceptance pair: Minecraft `1.21.1`, Fabric Loader `0.16.14`.

1. Resolve the exact pair with production HTTPS configuration.
2. Verify the component graph has exactly the Minecraft base plus Fabric Loader and no Forge-family
   loader.
3. Inspect normalized main class/libraries/arguments and confirm Fabric API is not implicitly added.
4. Install, restart Graphene, disable loader/provider access, and build the launch plan from
   committed state.
5. Launch and record bounded Fabric initialization evidence, then terminate normally.
6. Repeat in a second instance and verify ordinary immutable artifact reuse without processor output
   behavior.

A PASS requires actual upstream resolution, install, runtime launch, Fabric initialization evidence,
and restart/offline launch planning.

## Forge real smoke

**Status: NOT RUN.** Pinned acceptance pair: Minecraft `1.21.1`, Forge `52.1.0`.

1. Resolve the exact pair and record official installer identity/digest.
2. Verify the installer is acquired and integrity-verified before parsing.
3. Inspect the normalized component/patch and modern preparation recipe: typed data, explicit
   dependencies, client processors, allowlisted placeholders, and declared outputs.
4. Install and record the Java selected for processor requirements.
5. Confirm processors use synthetic staging/direct argv and cannot mutate immutable cache through
   writable hardlinks.
6. Verify generated output hashes/local SHA-256/provenance and atomic publication before instance
   commit.
7. Restart, disable provider/network access, build launch state from the receipt, launch, and record
   bounded Forge initialization evidence.
8. Install the same exact pair again and verify valid generated-output reuse; in an isolated test
   root corrupt reusable output/provenance and verify it is rejected/regenerated.

Legacy Forge metadata-only classification is not a modern-install smoke claim.

## NeoForge real smoke

**Status: NOT RUN.** Pinned acceptance pair: Minecraft `1.21.1`, NeoForge `21.1.200`.

1. Resolve the exact pair through the NeoForge adapter and acquire the installer/checksum through
   verified artifact acquisition before parsing.
2. Inspect exact component identity, normalized patch, and shared Forge-family preparation recipe.
3. Install with processor Java selected through the Java service and verify staging/direct-argv/
   timeout/output/cancellation boundaries.
4. Verify generated outputs/provenance before publication and final instance commit.
5. Restart with provider/network unavailable, build launch state only from committed normalized
   data, launch, and record bounded NeoForge initialization evidence.
6. Verify shared output reuse on a second install and corruption rejection/regeneration in a
   disposable isolated root.

## Modrinth content real smoke

**Status: NOT RUN.** Prerequisites: external public internet connection to `api.modrinth.com` and
live Minecraft game launch environment.

Procedure:

1. Create a supported Fabric instance (Minecraft `1.21.1`, Fabric Loader `0.16.14`).
2. Search Modrinth content via `graphene.content().search(modrinth, query)`.
3. Resolve an exact compatible mod version (e.g. Sodium `0.5.8`) and verify required dependencies.
4. Generate a `ContentMutationPlan` and verify non-mutating preview.
5. Execute the plan via `graphene.content().execute(plan)`.
6. Run `graphene.instances().verify(id, VerificationMode::Full)` and confirm healthy state.
7. Launch Minecraft and record bounded mod initialization log evidence.
8. Disable the network and verify local offline inventory scan and lockfile desired state remain
   intact.
9. Match local mod bytes via `graphene.content().recognize(id)` and confirm exact match.

## CurseForge content real smoke

**Status: NOT RUN.** Prerequisites: distributor-supplied CurseForge API key and live public network
connection to `api.curseforge.com`.

Procedure:

1. Configure a valid `CurseForgeProviderConfig` with `api_key` in `GrapheneBuilder`.
2. Search and resolve a compatible CurseForge mod file containing verified SHA-1 integrity.
3. Plan and execute installation through the journaled content mutation pipeline.
4. Run full instance verification and launch game.
5. Verify API key is redacted in all diagnostic, error, log, and persistent storage outputs.

## Recording rule

A smoke record may contain date, platform/architecture, exact public version identities, safe
hashes/operation stages, Graphene revision/delta, public account/profile identity where appropriate,
and pass/fail outcome. It must not contain tokens, device/user codes, secret store content, real
credentials, or unbounded logs.

## Modrinth pack import smoke

Procedure: acquire a real `.mrpack` with required + optional files; run
`Graphene::modpacks().inspect(...)` then `plan_import`/`execute_import` against a scratch data root;
launch the resulting instance.

**Status: NOT RUN.** Prerequisites: public internet access to `api.modrinth.com`/CDN and a
representative pack fixture.

## CurseForge pack import smoke

Procedure: same as above with a CurseForge manifest pack and a distributor API key configured;
verify exact `(project_id, file_id)` resolution and provenance persistence.

**Status: NOT RUN.** Prerequisites: CurseForge API key and live network.

## Prism/MultiMC instance import smoke

Procedure: export a standard MultiMC/Fabric instance, import it via the service, confirm embedded
mods are rejected-or-promoted per current support boundary and unsupported components fail typed.

**Status: NOT RUN.** Prerequisites: representative MultiMC export fixture (offline-capable).

## Graphene pack v1 round-trip smoke

Deterministic offline coverage exists at `crates/graphene-service/tests/modpack_export_roundtrip.rs`
(export → re-import → normalized assertions, staleness, create-only publication). A real-fixture
smoke over a large pack remains:

**Status: NOT RUN.** Prerequisites: representative large pack archive.

## Recording rule

Real smokes above are recorded only as PASS after execution against live services; absent that, they
remain NOT RUN with prerequisites stated. Offline deterministic suites are listed in the repository
verification gate and must stay green independently.
