# ADR-0005: Keep Process Ownership Behind Graphene Launch/Platform Boundaries

- **Status:** Accepted
- **Date:** 2026-08-08

## Context

Java probing and Minecraft launch require OS process primitives, but Tokio child handles, shell
construction, and unbounded output channels would leak implementation details or create deadlock and
memory-risk surfaces.

## Decision

`graphene-platform` owns private direct-process spawn/probe primitives. `graphene-java` uses them for
bounded Java probing. `graphene-launch` owns the public Minecraft lifecycle abstraction
(`RunningGame`, `GameEventStream`, `GameExit`) and converts an inspectable `LaunchPlan` to direct
argv only at execution.

Stdout and stderr are drained concurrently into a bounded host-facing queue. Output is dropped when
a slow consumer saturates the queue; dropped output is counted, and capacity is reserved for
terminal lifecycle delivery. Shell command strings are never constructed.

## Consequences

- public callers never own Tokio process handles;
- secret argv classification survives until the child-process boundary;
- slow event consumers cannot intentionally backpressure child stdout/stderr pipes indefinitely;
- process start/PID/output/exit/wait/kill semantics remain Graphene-owned and replaceable.

## Revisit Conditions

Revisit if later diagnostics require a different bounded fan-out/spooling design, while preserving
direct argv execution, secret redaction, and private process implementation types.
