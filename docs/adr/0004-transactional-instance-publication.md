# ADR-0004: Publish Instances as Create-Only Staged Transactions

- **Status:** Accepted
- **Date:** 2026-08-08

## Context

Installation must never leave a valid-looking committed instance after failure or cancellation.
Shared immutable artifacts may be acquired before an instance exists, while future instance work
needs durable, provider-neutral state that can be migrated rather than reparsed from provider DTOs.

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
- Future instance work can extend/migrate the versioned schemas without making Mojang DTOs
  authoritative state;
- final publication is intentionally a point of no return after the operation cancellation seal;
- The current install path remains create-only and does not prematurely implement general instance
  mutation locks.

## Revisit Conditions

Revisit when mutable instance transactions/migrations are introduced or when supported filesystems
require a stronger cross-filesystem publication strategy than same-root staged rename.
