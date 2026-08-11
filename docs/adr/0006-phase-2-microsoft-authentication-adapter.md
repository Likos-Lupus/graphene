# ADR-0006: Keep Microsoft Authentication as a Narrow Provider Adapter

- **Status:** Accepted for Phase 2 implementation candidate
- **Date:** 2026-08-11

## Context

Phase 2 needs UI-independent Microsoft device authorization followed by the Xbox/XSTS/Minecraft
services chain, entitlement verification, profile retrieval, and refresh. The stable domain cannot
own Microsoft DTOs, endpoints, or token semantics, and Graphene must not borrow another launcher's
application registration.

During implementation, the Microsoft identity device-code flow was checked against the current
Microsoft identity-platform device authorization documentation:
`https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-device-code`. The documented
identity boundary uses the v2 `/devicecode` and `/token` endpoints, provider polling interval/error
semantics, and `offline_access` when a refresh token is required. Microsoft guidance prefers
supported MSAL implementations generally, but this repository does not have a supported first-party
Rust MSAL dependency that satisfies the established Graphene dependency/boundary model.

The downstream Xbox/Minecraft authentication protocol is more volatile and is not treated as stable
Graphene domain configuration.

## Decision

Implement a narrow Microsoft adapter in `graphene-providers` behind the provider-neutral
`graphene_auth::AuthProvider` port.

- `MicrosoftAuthConfig::production` requires an explicit distributor-owned client ID, tenant,
  scopes, and `MicrosoftServiceEndpoints`.
- No client ID is bundled or borrowed.
- Identity device/token endpoints are derived from the configured tenant.
- `offline_access` is mandatory for persistent Microsoft accounts.
- Xbox/Minecraft endpoints and relying-party values remain isolated configuration rather than
  literals spread through services/domain code.
- Provider DTOs are private to the `microsoft` module tree.
- Credential-bearing protocol requests use the shared `NetworkClient`, no redirect following, no
  generic retry loop, and bounded response bodies.
- The adapter normalizes only the minimum account/session fields Graphene needs.

Local HTTP exists only through the explicit fixture constructor and loopback endpoint validation.

## Consequences

- `graphene-auth` stays provider-neutral and free of networking/storage/launch dependencies.
- `graphene-service` orchestrates normalized sessions but cannot parse Microsoft responses.
- Production distributors own application registration and volatile downstream service policy.
- Fixture tests can prove the vertical chain without real credentials/public services.
- This candidate is not a turnkey production Microsoft configuration: a distributor must supply the
  real application/service configuration and a real secure `SecretStore` backend.

## Revisit Conditions

Revisit if Microsoft ships and supports a Rust authentication library whose dependency/runtime model
fits Graphene without leaking provider types across the bounded context. The public
`AuthProvider`/`AuthSession` contract should remain unchanged.
