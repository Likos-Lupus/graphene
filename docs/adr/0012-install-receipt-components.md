# ADR-0012: Persist Exact Components in Install Receipt Schema 2

- **Status:** Accepted
- **Date:** 2026-08-12

## Context

Legacy schema-1 receipts persisted sufficient normalized Vanilla launch metadata but had no explicit
component set. Current receipts must preserve exact base Minecraft and loader identity for offline
launch, future repair, and content compatibility without parsing synthetic version IDs or
recontacting providers after commit.

Changing the JSON shape without a deliberate schema transition would break existing schema-1
instances.

## Decision

Bump `INSTALL_RECEIPT_SCHEMA_VERSION` from 1 to 2 and persist `components` containing exact UID,
version, kind, provider identity, and bounded safe provenance detail.

- Schema 2 validates exactly one Minecraft component, at most one loader, and consistency between
  `net.minecraft` and the receipt's resolved base version.
- New Vanilla receipts explicitly persist `net.minecraft <resolved-version>`.
- Schema-1 Vanilla receipts remain readable. Deserialization synthesizes the implicit Minecraft
  component in memory with Mojang/legacy provenance and preserves the existing normalized launch
  metadata.
- Reading an old receipt does not destructively rewrite it merely to launch.
- Raw provider responses, auth state, staging paths, and temporary processor data are not persisted.

## Consequences

Old Vanilla instances can continue offline launch planning, while current instances expose exact
loader identity directly to future repair/diff and content compatibility work. Provider availability
is not required after commit.
