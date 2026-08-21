# Contributing to Graphene

Graphene is a UI-independent launcher engine with deliberately strict boundaries. Keep changes
focused, preserve public/persisted behavior unless the change explicitly owns a compatibility
transition, and avoid mixing feature work with broad refactoring.

## Non-negotiable rules

- Keep the root and crate `lib.rs` files thin facades.
- Keep provider DTOs and transport/process implementation types behind their owning boundaries.
- Do not introduce forbidden dependency edges or UI framework dependencies into the engine.
- Preserve transaction, path-containment, integrity, cancellation, secret-redaction, and persistence
  guarantees.
- Add tests for durable behavior or risk, at the lowest effective level; do not widen public API for
  test convenience.
- Review hand-written production files at 500+ lines, expect decomposition at 800+, and split at
  1000+ unless a narrow explicit exception exists.
- Add or extend the one canonical document for a concern instead of creating a parallel policy or
  phase-owned document.
- Use ADRs only for durable, high-cost architectural decisions.
- Keep development-phase chronology in the roadmap rather than source/test/CI names.

Read [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md),
[`docs/ENGINEERING_STANDARDS.md`](docs/ENGINEERING_STANDARDS.md), and
[`docs/SECURITY.md`](docs/SECURITY.md) before changing boundaries or trust-sensitive code.

## Validation

Run the repository quality gate described in
[`docs/ENGINEERING_STANDARDS.md`](docs/ENGINEERING_STANDARDS.md). Keep local CI deterministic:
normal Cargo tests must not depend on the public internet. Real-provider/runtime smoke procedures
live in
[`docs/VALIDATION.md`](docs/VALIDATION.md).
