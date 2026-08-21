# ADR-0007: Fail Closed When No Secure Credential Store Is Supplied

- **Status:** Accepted
- **Date:** 2026-08-11

## Context

A persisted Microsoft account needs a refresh credential after restart, while public account JSON
must remain non-secret. OS credential-vault APIs are platform-specific and potentially blocking. A
weak plaintext fallback, hard-coded encryption key, or encryption key stored beside ciphertext would
violate the credential-storage threat model.

## Decision

`graphene-auth` defines a provider-neutral `SecretStore` port and `SecretRecordIdentity`. The record
key is derived from a Graphene namespace, record kind, and `AccountId`.

`graphene-service` accepts `Arc<dyn SecretStore>` through `GrapheneBuilder` and runs all backend
operations on blocking workers. The default is `UnavailableSecretStore`, which returns the stable
`AUTH_SECRET_STORE_UNAVAILABLE` failure.

The only bundled writable backend is `InMemorySecretStore::new_for_tests()`. Its naming and
constructor make fixture use explicit; it is not a persistence fallback.

Graphene implements **no plaintext fallback** and **no encrypted-file fallback**. It also does not
bundle a concrete OS credential-vault adapter. A production host/distributor must inject an
implementation that uses the platform's appropriate secure credential service. If it does not,
Microsoft persistence fails closed.

## Threat Model

The boundary protects against ordinary filesystem disclosure of the Graphene data root, accidental
serialization/logging of refresh credentials, and silent downgrade when a credential service is
unavailable. It does not claim to protect credentials from a process/user with authority to read the
host OS credential vault or memory of the running Graphene process.

An encrypted-file fallback may be added only with authenticated encryption, fresh nonces,
schema/versioning, atomic replacement, restricted permissions, a key not stored beside ciphertext,
and a documented host-secret/passphrase + memory-hard KDF design. No such fallback is enabled here.

## Consequences

- public account state and secrets are separate persistence systems;
- Graphene never silently writes the refresh credential to JSON;
- headless/CI environments remain deterministically testable with the explicit in-memory adapter;
- production Microsoft persistence is unavailable until a secure backend is injected;
- the absence of a bundled OS adapter remains an explicit deployment limitation.

## Revisit Conditions

Add platform adapters only when they can be compiled and exercised on their supported platforms
without changing `graphene-auth` or provider protocol APIs. Revisit encrypted fallback only after a
separate security design/review.
