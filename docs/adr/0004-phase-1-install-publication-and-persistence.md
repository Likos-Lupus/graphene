# ADR-0004: Publish Phase 1 Instances as Create-Only Staged Transactions

- **Status:** Accepted
- **Date:** 2026-08-08

## Context

Phase 1 must never leave a valid-looking committed instance after failure or cancellation. Shared
immutable artifacts may be acquired before an instance exists, while later phases need durable,
provider-neutral state that can be migrated rather than reparsed from provider DTOs.

## Decision

Installation separates shared immutable materialization from an isolated per-operation instance
staging directory. The complete `InstallPlan` is known and validated before staging. The executor
writes schema-versioned `instance.json` and `.graphene/install.json`, validates staged state, yields
a final cancellable pre-commit boundary, seals cancellation, and publishes the staging directory to
the final instance path with create-only/no-replace semantics.

The install receipt persists only Graphene-owned normalized launch inputs and managed-relative
paths. It does not persist provider DTOs, absolute host roots, or launch-session secrets.

## Consequences

- pre-commit failure/cancellation cannot masquerade as a successful instance;
- valid shared cache/materializations can survive a cancelled instance transaction;
- Phase 4 can extend/migrate the versioned schemas without making Mojang DTOs authoritative state;
- final publication is intentionally a point of no return after the operation cancellation seal;
- Phase 1 remains create-only and does not prematurely implement general instance mutation locks.

## Revisit Conditions

Revisit when Phase 4 introduces mutable instance transactions/migrations or when supported
filesystems require a stronger cross-filesystem publication strategy than same-root staged rename.
