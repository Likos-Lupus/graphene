# Graphene Engineering Standards

This document is the permanent repository-wide standard for tests, source modularity, documentation,
and architecture decision records. Architecture direction and trust boundaries remain owned by
[`ARCHITECTURE.md`](ARCHITECTURE.md) and [`SECURITY.md`](SECURITY.md).

## Testing policy

Tests exist to protect durable supported behavior or a durable risk boundary. A test is admitted
only when all of the following questions have satisfactory answers:

1. What supported contract or risk does it protect?
2. Would the test still be valid after a correct internal refactor?
3. Is it written at the lowest effective test level?
4. Is the same failure already protected elsewhere?
5. Is its execution and maintenance cost justified?

### Test categories

**Unit tests** cover small deterministic module behavior. Keep small blocks inline or place
substantial local suites in an adjacent `tests.rs`, for example `src/foo.rs` plus
`src/foo/tests.rs`. Private invariants stay private; do not widen visibility for test placement.

**Crate integration tests** cover public crate behavior that crosses modules, filesystem/process
boundaries, or local fixtures. Put them in `crates/<crate>/tests/*.rs`.

**Workspace/facade integration tests** are a deliberately small set of complete public Graphene use
cases under `tests/*.rs`. Name them by capability, not by development chronology.

**Architecture/static checks** live in `scripts/check_architecture.py` and
`scripts/check_hygiene.py`. They enforce durable repository invariants, not an exact snapshot of
today's implementation shape.

**Manual/external smoke validation** is not a normal Cargo test. Procedures and honest execution
status live in [`VALIDATION.md`](VALIDATION.md).

### What to preserve

Security, persistence compatibility, cancellation/terminal-state races, path containment, archive
bounds, secret redaction, hash verification, generated-output publication, and external-format
compatibility are durable protections. Old-data tests are valuable when old supported data must
remain readable or fail safely.

Prefer table-driven tests when only inputs and expected outcomes vary. Avoid custom test DSLs unless
they materially improve clarity.

### What not to test

Do not add tests or static checks whose sole purpose is to freeze:

- the exact current dependency set or dependency graph;
- private helper call order or implementation sequence;
- private struct layout;
- exact file/module existence without an architecture rule;
- phase/checklist completion or development-phase naming;
- duplicate branch-by-branch cases already covered at a lower level;
- large snapshots of private intermediate structures;
- public-internet availability in normal CI.

Stable serialized schemas and intentionally stable public normalized contracts may have focused
snapshot or golden-data coverage. Snapshot tooling is not otherwise a default.

## Source modularity

Line count is a review signal, not a substitute for cohesion. Measure hand-written production Rust
separately from large `#[cfg(test)]` modules.

| Production lines | Standard                                                                                    |
|-----------------:|---------------------------------------------------------------------------------------------|
|            0–499 | Normally acceptable.                                                                        |
|          500–799 | Mandatory decomposition review. Keep only when the responsibility remains cohesive.         |
|          800–999 | Split expected. A cohesion exception needs explicit justification in review.                |
|            1000+ | Split required unless a narrow exceptional allowlist is explicitly documented and enforced. |

Crate roots are stricter: they are facades and must satisfy the limit enforced by
`scripts/check_architecture.py`.

Split by responsibility and ownership, not by chunks. Names such as `part1`, `helpers2`, `misc`, or
an undifferentiated `utils` module are not acceptable decomposition. Prefer modules such as
`request`, `download`, `validation`, `inventory`, or `processor` that describe the behavior they
own. Avoid cyclic responsibility and avoid duplicating an existing policy/retry/verification
primitive merely to reduce a file's size.

When a file is large mainly because of tests, move substantial tests adjacent first, then review the
remaining production responsibility. Roughly 100–150 test lines, or a test block that dominates the
source file, is a useful trigger rather than a hard threshold.

## Documentation policy

Every permanent document must have one durable owner/responsibility. Prefer updating a canonical
document over adding another overlapping note.

Canonical concerns are:

- `README.md` — short project entry point and current capability/status summary;
- `PROJECT_SPECIFICATION.md` — product, scope, final target, bounded contexts, and non-goals;
- `ARCHITECTURE.md` — dependency direction and architectural boundaries;
- `ENGINEERING_STANDARDS.md` — this policy;
- `API.md` — current public behavior;
- `SECURITY.md` — current trust boundaries and residual risk;
- `VALIDATION.md` — automated gates and manual/external validation procedures/status;
- `LOADER_SUPPORT.md` — evidence-based loader compatibility;
- `ROADMAP.md` — development chronology and future phases;
- `adr/` — qualifying durable architectural decisions.

Do not create permanent cleanup diaries, file-split reports, test-count reports, duplicated policy
documents, or phase-owned implementation/API/security/smoke documents. Version control is the
history for completed implementation plans.

Links to source or other docs must be kept valid when files move or are consolidated. Current facts
belong in the current canonical owner rather than being copied into several documents.

## ADR admission policy

An ADR is appropriate when a decision is durable, crosses a meaningful architectural boundary, and
is expensive or risky to reverse. Typical examples include persistence compatibility, process or
security boundaries, domain ownership, dependency inversion, and transaction/publication semantics.

Do not create an ADR merely for:

- choosing an ordinary implementation dependency;
- selecting one reference provider behind an already-defined port;
- recording a local module/file placement;
- documenting a registry entry or helper location;
- preserving phase history.

Implementation-specific rationale belongs in code, manifests, or the canonical architecture/API/
security document. If an ADR no longer qualifies, transfer any still-durable rationale to its
canonical owner and remove or explicitly supersede the ADR.

ADR titles describe the decision, not the phase in which it was made.

## Required quality gate

Before delivery of repository changes, run when the toolchain/environment permits:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
python scripts/check_architecture.py
python scripts/check_hygiene.py
```

A gate that cannot run is `NOT RUN`, with the concrete reason recorded. A failing durable test is
not deleted simply to obtain a green result.
