# ADR-0011: Normalize Forge-Family Installers into Staged Java Preparation Recipes

- **Status:** Accepted for Phase 3 implementation candidate
- **Date:** 2026-08-12

## Context

Modern Forge-family installers describe Java processors that can transform inputs into runtime
artifacts. Running an opaque upstream installer directly against committed Graphene state would
bypass the existing artifact verifier, transaction boundary, cancellation/progress model, Java
selection, path safety, and future repair provenance.

Forge and NeoForge share enough processor semantics to justify one stable preparation executor, but
their discovery/versioning/integrity adapters remain distinct.

## Decision

Verified Forge/NeoForge installer metadata normalizes in `graphene-providers` to the
provider-neutral `ComponentPreparationRecipe` owned by `graphene-minecraft`.

- Installer JARs are acquired and integrity-verified by the existing `Artifact` pipeline before
  provider parsing.
- Archive inspection is bounded and selective; unsafe paths/types/expansion and unknown schema
  behavior fail closed.
- Raw data/placeholder syntax becomes typed data values and tokenized allowlisted arguments.
- Processor executable/classpath artifacts and embedded inputs are explicit before execution.
- Server-only processors are skipped because Graphene Phase 3 is a client launcher.
- `graphene-install` executes recipes only inside a synthetic writable staging tree. Verified cache
  objects are copied rather than exposed by writable hardlink.
- `InstallToolRunner` is a dependency-inverted port. The service adapter selects tool Java through
  the existing Phase 2 Java boundary and spawns direct argv with timeout, cancellation, and bounded
  output. No shell, script, or installer-directed native executable is supported.
- Only declared generated outputs can be published. Declared hashes are verified; locally generated
  SHA-256 is labeled local provenance. Shared publication is atomic and reuse requires matching
  digest/provenance.
- Provider/network calls are forbidden from install execution. A processor is not allowed to trigger
  hidden Graphene downloads.

## Executable-Code Limitation

Verified Java bytecode is still executable third-party code. Phase 3 does not claim JVM/OS
sandboxing. Its security boundary is trusted configured provider + verified artifacts + managed
staging + direct argv + bounded process controls + declared-output verification.

## Consequences

Forge can be installed without allowing its opaque installer to mutate a committed instance, and
NeoForge can reuse the same stable executor without being represented as a Forge configuration flag.
Future non-Java executable actions require a new explicit security decision rather than silently
expanding this port.
