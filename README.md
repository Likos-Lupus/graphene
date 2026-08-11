# Graphene

Graphene is a planned **UI-independent Minecraft: Java Edition launcher engine** written in Rust.

The repository contains the Phase 0 foundation, the Phase 1 Vanilla install-to-launch engine, and a
Phase 2 authentication/managed-Java **implementation candidate**. Tauri, Slint, CLI, and other hosts
are consumers of the engine rather than part of its domain. Phase 2 adds provider-neutral accounts,
offline/Microsoft session orchestration, separate secure-secret storage, and explicitly requested
managed Java while preserving the existing Phase 1 `LaunchSession` and `JavaRuntime` boundaries.

The Phase 2 candidate is not release-sign-off complete in this worktree: Cargo validation could not
run in the execution sandbox, the Phase 1 real Vanilla smoke remains `NOT RUN`, a production OS
credential-vault backend must be injected by the host/distributor, and the Phase 2 real auth/Java
smokes remain `NOT RUN`.

## Architecture Baseline

Start here:

- [`docs/PROJECT_SPECIFICATION.md`](docs/PROJECT_SPECIFICATION.md) — product definition, initial
  direction, final deliverable, architecture, boundaries, security, and Definition of Done.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — layer model, bounded contexts, and dependency
  rules.
- [`docs/SCOPE_AND_BOUNDARIES.md`](docs/SCOPE_AND_BOUNDARIES.md) — explicit ownership and non-goals.
- [`docs/ROADMAP.md`](docs/ROADMAP.md) — phased implementation order and exit criteria.
- [`docs/PHASE_0_IMPLEMENTATION_PLAN.md`](docs/PHASE_0_IMPLEMENTATION_PLAN.md) — detailed Phase 0
  foundation implementation plan, contracts, testing, CI, and exit criteria.
- [`docs/PHASE_1_IMPLEMENTATION_PLAN.md`](docs/PHASE_1_IMPLEMENTATION_PLAN.md) — normative Vanilla
  install-to-launch scope, workstreams, tests, security requirements, and exit criteria.
- [`docs/PHASE_1_API.md`](docs/PHASE_1_API.md) — implemented Phase 1 public behavior.
- [`docs/PHASE_1_SECURITY_REVIEW.md`](docs/PHASE_1_SECURITY_REVIEW.md) — Phase 1 security review and
  explicit validation limitations.
- [`docs/PHASE_1_SMOKE_TEST.md`](docs/PHASE_1_SMOKE_TEST.md) — required real Vanilla sign-off
  procedure and execution record.
- [`docs/PHASE_2_IMPLEMENTATION_PLAN.md`](docs/PHASE_2_IMPLEMENTATION_PLAN.md) — normative Phase 2
  authentication/managed-Java design and acceptance criteria.
- [`docs/PHASE_2_API.md`](docs/PHASE_2_API.md) — actual Phase 2 candidate public behavior.
- [`docs/PHASE_2_SECURITY_REVIEW.md`](docs/PHASE_2_SECURITY_REVIEW.md) — Phase 2 threat-boundary
  review and explicit remaining limitations.
- [`docs/PHASE_2_AUTH_SMOKE_TEST.md`](docs/PHASE_2_AUTH_SMOKE_TEST.md) and
  [`docs/PHASE_2_JAVA_SMOKE_TEST.md`](docs/PHASE_2_JAVA_SMOKE_TEST.md) — opt-in real smoke
  procedures and current `NOT RUN` records.
- [`docs/TARGET_DELIVERABLE.md`](docs/TARGET_DELIVERABLE.md) — what a complete Graphene backend must
  provide.
- [
  `docs/adr/0001-ui-independent-launcher-engine.md`](docs/adr/0001-ui-independent-launcher-engine.md) —
  initial architecture decision.

## Phase 2 Vertical Extensions

```text
Account -> AuthSession -> existing LaunchSession
Managed Java -> existing JavaRuntime
                     |
                     v
              Phase 1 LaunchPlan -> Process
```

`graphene.accounts()` owns account orchestration while concrete Microsoft protocol code stays in
`graphene-providers`. `graphene.java()` keeps Phase 1 local selection and adds side-effect-free
committed managed selection plus explicit ensure/install operations that reuse the existing verified
artifact pipeline. No loader/content/UI work is part of Phase 2.

## Phase 1 Vertical Slice

```text
Minecraft metadata
      |
      v
ResolvedMinecraft
      |
      v
InstallPlan
      |
      v
Transactional install
      |
      v
Java selection
      |
      v
LaunchPlan
      |
      v
Minecraft process
```

The Phase 1 implementation targets this complete path with frozen local fixtures, transactional
create-only publication, local Java probing, offline launch planning, direct process execution, and
bounded lifecycle/output events. Phase completion still requires every exit-checklist validation to
run successfully; documentation does not substitute for those executable gates.

## Architectural Rule of Thumb

> Stable domain models point inward; volatile providers and host frameworks stay outside.

See the project specification before adding new crates, providers, loaders, or host integrations.
