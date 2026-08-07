# Graphene Scope and Boundaries

## 1. Scope Rule

Graphene owns **non-UI launcher behavior** for Minecraft: Java Edition.

A useful ownership test is:

> If the behavior should work unchanged in Tauri, Slint, and a headless CLI, it probably belongs in
> Graphene. If it exists only to present or interact with a specific UI, it belongs in the host.

## 2. Graphene Owns

- Minecraft metadata and version resolution;
- instance lifecycle and isolation;
- installation, update, verification, and repair;
- artifact download requirements and verification;
- Java discovery, selection, and managed runtime policy;
- account/session behavior;
- loader/component resolution;
- content project/version/dependency normalization;
- content installation/update workflows;
- modpack normalization/import/export workflows;
- launch command planning;
- Minecraft process lifecycle;
- structured logs/events/diagnostics;
- storage schemas and migrations;
- secure handling of secrets;
- platform capability abstraction.

## 3. Host Owns

- windows, views, routes, dialogs, and forms;
- colors, icons, typography, theme, animation;
- localization strings;
- notifications;
- system tray UX;
- drag-and-drop UX;
- browser/webview presentation;
- clipboard UX;
- UI-side filtering/sorting state;
- front-end persistence that does not affect launcher semantics.

## 4. Provider Owns

A concrete provider adapter owns:

- endpoint URLs;
- authentication headers required by that provider;
- provider DTOs;
- pagination syntax;
- provider-specific rate-limit handling;
- conversion from provider data into Graphene domain data;
- provider capability declaration.

The rest of Graphene must not assume a provider offers a capability it does not declare.

## 5. Storage Owns

Storage adapters own:

- file serialization/deserialization;
- schema versions;
- migrations;
- cache/database persistence;
- secret-store implementation wiring.

Storage does not decide:

- which Java is compatible;
- which loader should be installed;
- whether a content dependency is required;
- what a launch classpath contains.

## 6. Network Owns

Network owns transport semantics:

- HTTP;
- proxies;
- retries;
- timeouts;
- cache validators;
- download concurrency;
- resume;
- source fallback.

Network does not know that an artifact is a Fabric loader or a Modrinth mod except through generic
metadata required for diagnostics/telemetry.

## 7. Explicit Non-Goals for Core

- UI framework integration code;
- launcher store/payment functionality;
- advertisements;
- arbitrary social features;
- built-in server hosting;
- unversioned native plugin ABI;
- automatic execution of arbitrary shell code;
- cloud synchronization as a baseline requirement;
- proprietary compatibility hacks that cannot be isolated behind adapters.

## 8. Boundary Violation Examples

### Wrong

```rust
// graphene-content
pub async fn search_modrinth(...) -> modrinth::SearchResponse
```

### Correct

```rust
pub async fn search(...) -> SearchResult<ContentProject>
```

### Wrong

```rust
// graphene-launch
tauri::Window::emit("launch_progress", ...)
```

### Correct

```rust
event_bus.publish(OperationEvent::...)
```

### Wrong

```rust
if provider_name == "curseforge" {
// special service behavior
}
```

### Correct

```rust
if provider.capabilities().contains(ContentCapabilities::UPDATE_LOOKUP) {
...
}
```

## 9. Change Rule

If a feature does not clearly belong to one bounded context, do not place it in `graphene-service`
by default.

First define:

- the domain concept;
- the owner;
- the interface crossing the boundary;
- the direction of dependency.

`graphene-service` coordinates contexts; it is not a dumping ground for unclear ownership.
