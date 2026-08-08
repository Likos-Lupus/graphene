# Graphene Architecture

This document is a focused architectural reference. The normative project definition is in
`PROJECT_SPECIFICATION.md`.

## 1. Layer Model

```text
Facade
  graphene

Application / Composition
  graphene-service

Use Cases
  graphene-install
  graphene-launch
  graphene-diagnostics

Domain Contexts
  graphene-minecraft
  graphene-instance
  graphene-auth
  graphene-java
  graphene-content
  graphene-pack

Adapters / Infrastructure
  graphene-providers
  graphene-storage
  graphene-network
  graphene-platform

Foundation
  graphene-core
```

The layers describe dependency direction, not runtime call order.

## 2. Dependency Rules

- Higher layers may orchestrate lower-layer interfaces.
- Stable domain crates do not depend on concrete provider integrations.
- Adapter crates may depend on domain crates to normalize external data.
- `graphene-service` wires implementations together.
- the root `graphene` crate exports a curated stable API and contains no duplicated business logic.
- cycles between crates are forbidden.

## 3. Bounded Context Ownership

| Crate                  | Primary responsibility                                    | Explicitly not responsible for    |
|------------------------|-----------------------------------------------------------|-----------------------------------|
| `graphene-core`        | IDs, artifacts, operations, progress, errors, diagnostics | Minecraft/provider/UI logic       |
| `graphene-platform`    | OS/filesystem/process/keyring capabilities                | launcher domain policy            |
| `graphene-network`     | HTTP/download/cache/retry/mirror behavior                 | provider normalization            |
| `graphene-minecraft`   | Minecraft metadata and resolution                         | network/UI/process execution      |
| `graphene-auth`        | accounts, sessions, auth provider contracts               | UI windows/keyring implementation |
| `graphene-java`        | runtime discovery/probe/selection                         | Minecraft metadata parsing        |
| `graphene-content`     | normalized content model/provider contracts               | provider-specific DTOs            |
| `graphene-instance`    | instance identity/layout/config/lifecycle                 | downloading/launching             |
| `graphene-pack`        | pack formats and normalization                            | separate installer                |
| `graphene-providers`   | concrete external APIs and DTO mapping                    | stable domain policy              |
| `graphene-install`     | plans, transactions, repair execution                     | UI/provider DTOs                  |
| `graphene-launch`      | launch plan and game process lifecycle                    | auth/provider HTTP                |
| `graphene-diagnostics` | verification/crash/log diagnostics/redaction              | localized UI prose                |
| `graphene-storage`     | persistence and migration                                 | domain decision-making            |
| `graphene-service`     | application orchestration and DI                          | reimplementation of contexts      |

## 4. Core Plan Model

### Installation

```text
Request -> Resolve -> Plan -> Stage -> Fetch -> Verify -> Materialize -> Validate -> Commit
```

No complex instance mutation is allowed to skip planning and transaction boundaries.

### Launch

```text
Request -> Resolve instance -> Resolve account -> Resolve Java
        -> Resolve Minecraft -> Build LaunchPlan -> Execute
```

`LaunchPlan` is deterministic input to process execution.

## 5. Adapter Boundary

External data is normalized immediately:

```text
External JSON/HTTP
      |
      v
Provider adapter
      |
      v
Graphene domain model
      |
      v
Services / Plans / UI hosts
```

External DTOs may be retained inside `graphene-providers` for caching or debugging, but are not
public service-layer types.

## 6. Host Boundary

Allowed:

```text
Tauri app -> graphene
Slint app -> graphene
CLI       -> graphene
```

Forbidden:

```text
graphene-minecraft -> tauri
graphene-launch    -> slint
graphene-auth      -> desktop window callbacks
```

## 7. Concurrency and Operations

Every long-running use case receives an `OperationId` and cancellation token and emits structured
operation events.

An instance mutation obtains an instance-level write lock.

Read-only metadata and content operations should remain concurrent where safe.

## 8. Persistence Boundary

Authoritative state:

- launcher config files;
- instance metadata;
- Graphene lockfiles;
- user content.

Disposable/rebuildable state:

- HTTP cache;
- provider search cache;
- hash lookup cache;
- task history;
- derived database indexes.

## 9. Dependency Review Checklist

Before merging a new dependency between crates, answer:

1. Which context owns the type being shared?
2. Is this dependency from stable domain to volatile adapter?
3. Could an interface/value object invert the dependency?
4. Is a provider-specific type crossing the boundary?
5. Would adding a second provider force a change in the consumer?
6. Does this make a UI framework part of backend compilation?
7. Does it create or approach a dependency cycle?

If any answer indicates boundary leakage, redesign before merging.

## 10. Phase 1 Activated Dependency Graph

Phase 1 activates the Vanilla install-to-launch contexts without changing the inward dependency
rule. The mechanically enforced internal graph is:

```text
graphene-minecraft -> graphene-core

graphene-instance  -> graphene-core

graphene-platform  -> graphene-core

graphene-network   -> graphene-core

graphene-java      -> graphene-core + graphene-platform

graphene-providers -> graphene-core + graphene-network + graphene-minecraft

graphene-storage   -> graphene-core + graphene-platform

graphene-install   -> graphene-core + graphene-minecraft + graphene-instance
                   + graphene-storage + graphene-platform

graphene-launch    -> graphene-core + graphene-minecraft + graphene-instance
                   + graphene-java + graphene-platform

graphene-service   -> all activated backend contexts for composition only

graphene (facade)  -> curated Graphene-owned context/service APIs
```

`graphene-install` owns the `ArtifactAcquirer` port; `graphene-service` implements the adapter to
the Phase 0 `ArtifactService`. This keeps the verified transport/cache pipeline reusable without a
reverse service dependency. See ADR-0003.

Phase 1 committed instances use a staged create-only publication transaction and a provider-neutral
schema-versioned install receipt. See ADR-0004. Java/Minecraft processes are direct-argv and their
Tokio implementation handles remain private. See ADR-0005.

The architecture checker rejects direct Reqwest dependencies outside `graphene-network`, provider
DTO leakage, backend UI dependencies, forbidden Phase 1 edges, cycles, and root-facade provider
parsing.
