# Phase 3 Security Review

This review records the Phase 3 loader-component implementation candidate against the documented
trust boundaries. It distinguishes source-level controls and deterministic fixtures from executable
validation. The current environment has no Rust toolchain, so Cargo checks, Clippy, Rustdoc,
cross-platform CI, and real loader smoke tests are **not claimed as passed**.

## Dependency and Trust Boundaries

- `graphene-minecraft` owns component/patch/loader-preparation domain types and has no provider,
  network, service, or install dependency.
- Fabric/Forge/NeoForge network/DTO logic is private to `graphene-providers`.
- `graphene-install` owns preparation execution but has no provider, network, or service dependency.
- `graphene-launch` consumes committed normalized receipt state and has no provider/auth/loader
  implementation dependency.
- Loader providers never receive account/authentication state or token-bearing types.
- Processor Java selection is inverted through `InstallToolRunner`; providers do not call Adoptium
  or use ambient PATH as an implicit requirement resolver.

The source architecture guard enforces these dependency directions, DTO privacy, thin crate roots,
and no direct Reqwest in Minecraft/install loader code.

## Metadata, Endpoint, and Integrity Policy

Loader metadata reads are bounded before parsing. Production provider configurations require HTTPS,
reject userinfo/`file:` URLs and direct localhost/private literal endpoints, and use the existing
network redirect/TLS policy. Explicit fixture constructors are the only HTTP exception.

Remote executable/runtime artifacts require provider-supplied integrity before they become normal
Graphene `Artifact` values:

- Fabric uses profile integrity or official Maven SHA-1 sidecars where needed;
- Forge requires the official Maven installer SHA-1 sidecar;
- NeoForge requires the official Maven installer SHA-256 sidecar;
- processor/classpath Maven artifacts must have normalized trustworthy integrity from the verified
  installer/profile data before execution.

These trust classes remain distinct from embedded data contained by an already verified installer
and from locally derived SHA-256 for generated output. A local digest is never described as upstream
verification.

## Installer Archive Boundary

Forge-family installer JARs are treated as untrusted executable archives even after transport hash
verification. The parser applies independent bounds for archive size, entry count, entry-name
length, individual expanded entry size, selected expanded bytes, and compression expansion ratio.
The ZIP/DEFLATE byte codec is the same bounded filesystem-neutral primitive used by prior phases;
Phase 3 does not maintain a second loader-specific decompressor. Provider code still owns its
stricter installer limits, selected-entry allowlist, and loader-specific error classification.

Before reading selected metadata, archive entries are validated to reject absolute paths, `..`
traversal, Windows drive/prefix escapes, backslash path ambiguity, symlinks, special-file semantics,
duplicate selected metadata entries, implausible expansion, and oversized entries. Only supported
metadata files and explicitly referenced embedded inputs are read; Graphene does not extract the
whole installer for inspection.

Frozen hostile fixtures cover traversal, absolute paths, oversized metadata/embedded data, excessive
entry count, Unix symlink/special-file metadata, malformed JSON, duplicate profile metadata, unknown
placeholders, output escapes, NUL arguments, script references, unsafe repositories, unsupported
specs, and base-version mismatch. The tests are present but could not be executed in this
environment.

## Profile and Placeholder Normalization

Raw Forge-family profile data stays private to the provider adapter. Known modern processor profiles
normalize into typed preparation values and tokenized arguments. Legacy launcher-profile data is
classified separately; malformed modern metadata never silently falls back to legacy interpretation.

The execution layer only expands an explicit allowlist equivalent to `ROOT`, `INSTALLER`,
`LIBRARY_DIR`, `MINECRAFT_JAR`, `MINECRAFT_VERSION`, `SIDE`, Maven-derived paths, and known typed
data variables. Unknown/missing placeholders, NULs, excessive argument count/length, or a managed
path that escapes the synthetic staging roots fail before spawning Java. There is no
environment-variable expansion and no shell-string reconstruction.

## Process Execution

Phase 3 does **not** provide a JVM or operating-system sandbox for loader processors. A verified
processor JAR is still executable third-party code. The implemented guarantee is limited to:

```text
configured trusted provider
+ verified executable inputs
+ isolated managed staging
+ selected Java runtime
+ direct argv/classpath execution
+ bounded timeout/output/cancellation
+ declared-output validation/publication
```

Graphene resolves a bounded `Main-Class` from the verified processor JAR manifest when the
normalized schema does not provide one explicitly. It invokes the selected Java executable directly
with a classpath and individual argv tokens. It does not run `sh`, `bash`, `cmd.exe`, PowerShell,
`.bat`,
`.cmd`, `.ps1`, arbitrary scripts, or installer-directed native executables.

The concrete service runner clears the inherited process environment and forwards only a small OS
execution allowlist. Microsoft/Xbox/Minecraft tokens are not placed in processor argv, environment,
staging files, events, or errors. Stdout/stderr are always drained concurrently into bounded lossy
UTF-8 buffers with truncation markers, preventing pipe deadlock and unbounded error strings.

Every processor has a finite timeout. Timeout/cancellation requests termination, bounds the wait for
process death, drains/closes captured streams, returns a structured loader-processor error, and
leaves final instance publication uncommitted.

## Staging and Shared Cache Immutability

Loader processors operate under a synthetic operation staging tree, never the committed Graphene
data root. Verified cached artifacts are **copied** into writable processor staging; writable
hardlinks to canonical immutable cache objects are not provided. This prevents a processor from
modifying the shared artifact cache through a staging alias.

Embedded installer inputs are selectively extracted from the verified installer into managed staging
and locally SHA-256 hashed for provenance. Server-only processors and their server-only embedded
data are not executed/materialized by the client launcher path.

## Generated Output Publication

Only outputs declared by the normalized recipe are eligible for publication. Staging and final paths
are classified and containment-checked. Arbitrary home/config/secret/other-instance destinations are
not representable through a valid managed output target.

An output with an upstream digest is hashed and rejected on mismatch. An output without an upstream
digest receives a local SHA-256 only after path/type/size checks. All generated outputs are rejected
above the Phase 3 2 GiB file-size bound even when upstream omits an expected size. Shared immutable
publication uses a temporary destination and the existing safe atomic replacement primitive;
unverified bytes are never written directly to the canonical target.

Reuse requires a matching bounded provenance record plus a fresh digest check of the canonical file.
Corrupt or provenance-incompatible output is regenerated. When reused output is needed by a later
processor, Graphene copies it back into private staging rather than granting the processor writable
access to shared state.

Concurrent duplicate producers may still perform redundant computation in this candidate. Final
publication remains filesystem-safe and each later reuse is digest/provenance validated;
optimization of duplicate in-flight processor work is not treated as a correctness guarantee.

## Cancellation and Transactional Commit

Component/provider resolution and artifact acquisition reuse operation cancellation. Installer
parsing/extraction, processor execution, output verification, and staging work check cancellation
before the final publication boundary. Component graph errors/cycles/conflicts fail before committed
mutation.

Instance installation retains the existing staged create-only transaction. The final cancellation
seal remains publication-specific; failure or cancellation before commit cannot create a
valid-looking committed instance. Generated shared objects may already have been safely published
before an instance commit and are independently verified/reusable immutable artifacts.

## Receipt and Offline Trust

Receipt schema 2 persists exact normalized components and final launch metadata, not raw provider
JSON. Schema-1 Vanilla receipts migrate only in memory to an implicit Minecraft component,
preserving prior launch semantics without network or destructive rewrite.

After commit, loader providers, installer parsers, and processor logic are not needed to plan
launch. The launch boundary therefore does not reintroduce provider/network trust during offline
planning.

## Residual Risk and Remaining Acceptance Work

1. Java processor bytecode is not sandboxed; a trusted verified processor can still exercise the
   permissions of the Graphene process account within OS controls.
2. Production endpoint checks reject explicit private/localhost literals but are not a complete DNS
   rebinding sandbox; the existing network redirect policy remains part of the boundary.
3. Legacy Forge launcher-profile execution is not Tier A in this candidate.
4. Concurrent identical generated-output requests may duplicate processor work even though atomic
   publication and reuse validation preserve correctness.
5. Cargo compilation/tests/format/Clippy/Rustdoc and Linux/Windows/macOS CI have not run here
   because
   `cargo`/`rustc` are unavailable.
6. Real Fabric, Forge, NeoForge runtime smoke tests remain `NOT RUN`.
7. Prior Phase 1 Vanilla and Phase 2 auth/managed-Java real smokes remain `NOT RUN`.
