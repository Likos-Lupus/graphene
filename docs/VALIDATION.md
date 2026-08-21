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

| Gate                                                    | Status  | Evidence/reason                                                   |
|---------------------------------------------------------|---------|-------------------------------------------------------------------|
| `cargo fmt --all -- --check`                            | NOT RUN | `cargo`/`rustfmt` are not installed in the execution environment. |
| `cargo check --workspace --all-targets`                 | NOT RUN | `cargo`/`rustc` are not installed.                                |
| `cargo test --workspace`                                | NOT RUN | `cargo`/`rustc` are not installed.                                |
| `cargo clippy --workspace --all-targets -- -D warnings` | NOT RUN | `cargo`/Clippy are not installed.                                 |
| `cargo doc --workspace --no-deps`                       | NOT RUN | `cargo`/`rustc` are not installed.                                |
| `python scripts/check_architecture.py`                  | PASS    | Executed against the current repository tree.                     |
| `python scripts/check_hygiene.py`                       | PASS    | Executed against the current repository tree.                     |

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

## Recording rule

A smoke record may contain date, platform/architecture, exact public version identities, safe
hashes/operation stages, Graphene revision/delta, public account/profile identity where appropriate,
and pass/fail outcome. It must not contain tokens, device/user codes, secret store content, real
credentials, or unbounded logs.
