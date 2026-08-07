# Graphene Target Deliverable

## 1. What a Consumer Receives

A consumer depends primarily on the root `graphene` crate and receives a configured `Graphene`
service facade.

```rust
let graphene = Graphene::builder(data_root)
.build()
.await?;
```

The facade exposes capability-oriented services:

```text
accounts
instances
minecraft
java
content
install
launch
diagnostics
```

## 2. Consumer Contract

A host should be able to implement a launcher without:

- calling Mojang APIs directly;
- implementing Microsoft session refresh;
- parsing Minecraft version JSON;
- calculating Maven classpaths;
- selecting natives;
- choosing Java by ad-hoc version checks;
- constructing JVM/game arguments;
- managing download retries and hash verification;
- writing a second modpack installer;
- parsing provider-specific content DTOs;
- creating shell command strings.

## 3. Observable Products of the Engine

The most important values Graphene returns are not UI widgets; they are deterministic engine
products:

- `ResolvedMinecraft`;
- `InstallPlan`;
- `RepairPlan`;
- `LaunchPlan`;
- `JavaSelection`;
- normalized account/session data;
- normalized content models;
- normalized pack manifests;
- structured diagnostics;
- operation/event streams.

These values should be serializable or debug-exportable where doing so is safe.

## 4. Version 1.0 Minimum Backend Capability

Version 1.0 should include:

- multi-instance model;
- Vanilla;
- Fabric;
- NeoForge;
- Forge;
- Microsoft and offline authentication;
- automatic Java discovery/selection;
- transactional install/repair;
- lockfile verification;
- launch planning/process management;
- Modrinth and CurseForge content;
- local mod management;
- `.mrpack` plus at least one additional major pack format;
- structured errors/diagnostics;
- progress/cancellation;
- secure secret handling;
- platform abstractions for Windows/Linux/macOS;
- comprehensive fixture tests.

## 5. What Version 1.0 Does Not Require

Version 1.0 does not require:

- every historical loader;
- every launcher pack format;
- a finished Tauri and Slint UI;
- dynamic third-party plugins;
- cloud sync;
- launcher self-update;
- advanced AI-like crash explanations;
- every convenience feature present in every competing launcher.

Those features can be added after the architecture and public contract are proven.
