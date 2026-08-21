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

Unknown user files are not treated as disposable Graphene state merely because they share the data
root.

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

## Persistence

Schema-versioned receipts/descriptors are validated on read. Compatibility migrations preserve old
supported data without treating malformed/unknown state as current. Receipt schema 1 remains
readable as Vanilla; current receipts persist explicit components.

Persisted state intentionally excludes raw provider DTOs, secrets, absolute data roots, transient
staging paths, and unbounded external response data.

## Residual risk and validation status

Deterministic tests cover many trust boundaries, but this worktree has not been release-sign-off
validated with the Rust toolchain or real upstream/runtime smokes because `cargo`/`rustc` are absent
from the execution environment. Real validation status is recorded only in
[`VALIDATION.md`](VALIDATION.md); fixture success must not be presented as production smoke success.
