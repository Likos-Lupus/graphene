# Graphene Security

This document records current trust boundaries, guarantees, and residual risks. Security-sensitive
behavior is also protected by focused tests; engineering/test policy lives in
[`ENGINEERING_STANDARDS.md`](ENGINEERING_STANDARDS.md).

## Network boundary

- Production transport uses Reqwest with Rustls TLS; there is no ordinary switch that disables
  certificate validation.
- Redirect count is bounded. HTTPS downgrade, unsafe schemes, missing hosts, and URL userinfo are
  rejected at protected metadata/artifact boundaries.
- Credential-bearing protocol requests use a no-redirect client so bearer/form credentials are not
  forwarded through 307/308 redirects.
- Metadata/protocol responses are bounded in memory; artifacts stream to disk under bounded
  concurrency/retry policy.
- Errors/tracing avoid full artifact URLs, explicit proxy secrets, bearer tokens, device codes, and
  provider response bodies.
- Local HTTP exists only through explicit fixture/provider configuration for deterministic tests.

Residual risk: standard hostname/DNS/TLS semantics apply; Graphene does not claim to provide a
network sandbox or universal SSRF defense for every future configurable endpoint. Provider endpoint
configuration remains a trust decision.

## Filesystem boundary

- Graphene operates under an explicit managed data root.
- Security-sensitive managed paths are lexical/canonical containment checked; parent symlink swaps
  and managed-root escapes are rejected.
- Temporary transfer/staging paths are Graphene-owned and created separately from final committed
  paths.
- Final cache/instance/runtime publication is staged and uses cooperating/atomic no-replace or safe
  replacement semantics appropriate to the operation.
- Persisted instance/runtime paths are relative managed paths rather than absolute host paths.
- Recursive clone, verify, and delete directory walkers strictly reject symbolic links, FIFOs,
  sockets, and character/block devices.
- Instance clone streams all file copies in bounded chunks and never creates ambiguous mutable
  hard-link or reflink aliases with the source.
- Instance delete atomically renames the instance tree into quarantine trash
  (`instances/.trash/<id>-<op_id>`) on the same filesystem before recursive cleanup, ensuring
  deletion commits immediately without race conditions.
- Lockfiles and repair plans are treated as untrusted input with strict size bounds (e.g. 1 MiB
  lockfile, 64 KiB config, 64 KiB descriptor) and revalidated source URLs.

Unknown user files are not treated as disposable Graphene state merely because they share the data
root. User-owned content under `.minecraft` (worlds, screenshots, custom logs) is never scanned or
hashed during verification unless explicitly declared in desired state.

## Archive boundary

The shared byte-level ZIP/DEFLATE codec is centralized in `graphene-core::archive` and accepts
caller-supplied entry/name/output limits plus cancellation. It does not choose filesystem paths or
write files.

Install extraction and Forge-family installer inspection add their own policy:

- traversal/absolute/unsafe names fail closed;
- symlink/special-file archive entries are rejected where unsupported;
- entry count, metadata size, per-entry expansion, and total output are bounded;
- duplicate/ambiguous critical entries are rejected;
- archive data is not trusted merely because a filename looks expected.

## Authentication and secrets

- Secret-bearing strings use `SensitiveString` and redact `Debug`/`Display`.
- Public account metadata is separate from refresh/access credentials.
- Persistent secrets go through the injected `SecretStore` port.
- The default builder uses an unavailable store; Graphene does not silently fall back to plaintext
  credential persistence.
- Microsoft device/user codes, access tokens, refresh tokens, Xbox/XSTS tokens, and credential
  response bodies must not be placed in normal logs/errors/diagnostics.
- Launch sessions are ephemeral and persisted install/instance schemas do not contain account
  tokens.

Residual risk: Graphene does not bundle an OS credential-vault backend in this repository. A
production host/distributor must inject one appropriate to its platform and threat model.

## Process boundary

- Java probes, install tools, and Minecraft execute a selected executable with direct argv; shell
  command construction is prohibited.
- Process implementation handles remain private behind Graphene-owned lifecycle APIs.
- Probe/tool execution has bounded timeout/output behavior; Minecraft output is drained concurrently
  through a bounded event path.
- Secret launch arguments remain classified/redacted until the process boundary.

Residual risk: once a secret is passed to the intended child process it is visible according to the
operating system's normal process model; Graphene does not claim to defeat a hostile same-user OS
observer.

## Managed Java boundary

Managed Java release metadata is normalized behind a distribution-provider port. Installation:

1. requires verifiable artifact integrity (current reference-provider flow uses SHA-256);
2. acquires through the existing verified artifact pipeline;
3. extracts only to managed staging under archive bounds/path policy;
4. rejects symlinked/unsafe executable paths;
5. probes the staged Java executable before publication;
6. persists a validated relative descriptor; and
7. publishes an immutable runtime only after final validation/cancellation sealing.

Committed runtimes are revalidated/probed when selected; descriptor identity or probe drift is
reported as corruption rather than silently trusted.

## Loader processor boundary

Forge-family installer bytes are parsed only after verified acquisition. Raw profile syntax
normalizes into typed data, explicit artifacts, allowlisted placeholders, client-side processor
steps, and declared generated outputs.

Install-tool execution:

- runs only explicit Java/JAR inputs selected through the Java boundary;
- uses direct argv, no shell/script interpreter;
- uses synthetic managed staging rather than writable access to immutable cache objects;
- has bounded timeout/output/cancellation behavior;
- cannot initiate hidden Graphene provider/network acquisition during execution;
- publishes only declared generated outputs after expected-hash/local-SHA-256 and provenance checks.

Residual risk: verified Java bytecode is still executable third-party code. Graphene does **not**
claim JVM or operating-system sandboxing. The trust model is configured provider + verified
artifacts + managed staging/process controls + declared-output verification.

## Generated artifacts

Generated output identity includes its declared target plus input/provenance identity. Publication
is atomic and reuse requires matching verified content/provenance rather than file existence.
Cancellation before publication, output path escape, missing/extra undeclared outputs, and hash
mismatch must not produce a reusable canonical object.

## Content and mod security

Content management enforces strict integrity, path safety, privacy, and secret boundaries:

- **Untrusted provider metadata**: Response payloads, pagination limits, and string fields are
  strictly bounded in memory. Metadata never executes scripts, native commands, or arbitrary Java.
- **Download integrity**: Remote mod files require verified SHA-1 or SHA-256 cryptographic digests
  before publication into instance desired state. MD5 and Murmur2 fingerprints are strictly lookup
  metadata and never satisfy artifact integrity.
- **Local JAR inspection**: Scanning `.minecraft/mods` is non-executing and non-extracting. Mod
  descriptors (`fabric.mod.json`, `mods.toml`, `neoforge.mods.toml`, `mcmod.info`) are parsed with
  bounded memory and entry limits using the centralized `graphene-core::archive` codec.
- **Filename and path containment**: Remote filenames are treated as untrusted metadata and
  sanitized against directory traversal (`..`), path separators (`/`, `\`), control characters, and
  case-folding collisions on case-insensitive filesystems.
- **Privacy and offline scanning**: `scan()` is strictly offline and never transmits hashes or
  fingerprints over the network. File matching/recognition occurs only upon explicit host request.
- **Secret redaction**: CurseForge API keys are wrapped in `SensitiveString`, redacted in `Debug`
  and tracing output, and never persisted to the data root, lockfile, journal, or diagnostics.
- **Transaction trust boundary**: The mutation journal (`.graphene/content-journal.json`) records
  filesystem transition mechanics only; it contains no credentials, authorization headers, or raw
  provider bodies.

## Persistence

Schema-versioned receipts/descriptors are validated on read. Compatibility migrations preserve old
supported data without treating malformed/unknown state as current. Receipt schema 1 remains
readable as Vanilla; current receipts persist explicit components.

Persisted state intentionally excludes raw provider DTOs, secrets, absolute data roots, transient
staging paths, and unbounded external response data.

## Diagnostic boundary

Local logs, crash reports, `hs_err` files, and instance metadata are untrusted input:

- **Read-only collection**: `DiagnosticService` holds the shared instance lease and inspects only a
  conservative allowlist (instance metadata/receipt/lockfile/config, `logs/latest.log`,
  `logs/debug.log`, bounded crash reports, bounded `hs_err_pid*.log`, offline mod inventory, and
  optional caller exit evidence). It never recursively sweeps `.minecraft`.
- **Bounded reads**: per-source and aggregate byte ceilings, bounded line length, bounded crash/
  `hs_err` selection, bounded excerpts, and bounded finding/evidence counts are named constants
  enforced before allocation.
- **Path containment**: subjects that are symlinks or non-regular files are rejected, and a
  subject's canonical parent must remain inside the instance root.
- **Hostile text**: lossy UTF-8 decoding and panic-free line scanning tolerate CRLF/LF, invalid
  UTF-8, truncated files, and oversized single lines. Parsing uses substring rules rather than a
  regex engine, so there is no rule-ReDoS surface.
- **Redaction before exposure**: known secret values, bearer/authorization tokens, credential
  key/value forms, URL userinfo/query credentials, and the explicit data root/home prefixes are
  redacted deterministically and idempotently before excerpts, serialization, or tracing. A
  redaction failure fails closed. This is secret redaction, not complete personal-data
  anonymization.
- **No network, no shell**: diagnostics performs no upload, no shell execution, and no
  parser-triggered command. Heuristic findings never authorize destructive remediation; every
  recommendation is non-mutating and must be invoked explicitly through its owning service.

## Residual risk and validation status

Deterministic tests cover many trust boundaries, and the automated quality gate (formatting,
workspace check/test/clippy/doc, architecture, and hygiene) passes with a Rust toolchain present.
Real upstream/runtime smokes have not been release-sign-off validated; real validation status is
recorded only in [`VALIDATION.md`](VALIDATION.md), and fixture success must not be presented as
production smoke success.

## Modpacks

Pack input is treated as untrusted archive data. Sources are pinned into the content-addressed cache
before parsing; large-pack paths stream through bounded readers instead of full-buffer reads. Entry
counts, name bytes, sizes, total expansion, and manifest bodies are bounded;
traversal/absolute/drive/symlink/special/collision forms fail closed. Pack-declared URLs pass a
strict HTTPS structural policy, and persisted lockfile state stores no raw credentials or signed
URLs. No pack metadata can request shell, native, or installer execution: import has no execution
fields, and unknown manifest content fails closed.

Graphene pack export excludes launcher-internal state (`.graphene/`, descriptor/receipt paths),
requires an explicit host embedding decision per destination, persists provenance as bounded
provider references rather than raw URLs where available, and publishes create-only after
re-validating the archive against the import contract. Residual risk: real-world interop smokes
remain NOT RUN (see VALIDATION.md).

## Reference host boundary

Reference hosts (`apps/`) consume only the root `graphene` facade. Two host trust boundaries are
introduced:

- **OS credential vault.** The reference secure-store adapter implements Graphene's existing
  `SecretStore` contract using the platform credential vault (macOS Keychain, Windows Credential
  Manager, Secret Service on non-macOS `*nix`) under the stable `SecretRecordIdentity::key()`
  namespace. There is no plaintext fallback: if the backend is unavailable, reads/writes fail
  explicitly, and offline-account workflows continue without it. Vault calls run through the
  engine's existing blocking-boundary secret paths, never on an async I/O worker. The adapter is
  host-owned; no OS-credential API enters `graphene-auth`.
- **Tauri IPC.** Tauri commands return app-local wire DTOs and a safe error envelope; refresh/access
  tokens, `LaunchSession`, and `RefreshCredential` values are never serialized to the webview. The
  only authentication material crossing IPC is the intended device-authorization display pair
  (verification URI + user code). Internal source errors, absolute secret paths, and implementation
  handles are omitted. Frontend logs never receive engine secret values, and diagnostic excerpts are
  already redacted by the engine before a host displays them.

Host shutdown terminates tracked games explicitly so an instance shared lease is never dropped while
a child process is alive. Residual risk: real OS-vault and packaged-desktop-host smokes remain NOT
RUN (see VALIDATION.md).
