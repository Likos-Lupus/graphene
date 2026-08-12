# ADR 0013: Share the bounded ZIP/DEFLATE byte codec through graphene-core

Status: Accepted

## Context

Phase 1 introduced a bounded Stored/Deflate ZIP reader in `graphene-install` for native archives.
Phase 2 reused the same semantics for managed runtime ZIPs. Phase 3 initially copied the low-level
ZIP/DEFLATE implementation into `graphene-providers` so Forge-family adapters could inspect an
already verified installer JAR without depending on `graphene-install`.

That copy duplicated security-sensitive parsing logic. Making `graphene-providers` depend on
`graphene-install` would invert the documented provider/install boundary, while introducing a new
loader-specific crate would be disproportionate.

## Decision

The filesystem-neutral ZIP central-directory parser, Stored/Deflate entry decoder, bounded raw
DEFLATE decoder, and CRC32 primitive live once under the shared `graphene-core::archive` module.

The shared codec:

- parses bytes only;
- accepts caller-supplied entry/name/output limits;
- observes the existing cancellation token while decompressing;
- does not open files, choose paths, extract directories, publish outputs, or decide trust policy;
- returns a small codec error that callers map into their own stable Graphene error family.

`graphene-install` continues to own native/runtime extraction, managed-path containment, filesystem
writes, and install error classification. `graphene-providers` continues to own verified
Forge-family installer/profile inspection, selected-entry policy, provider-specific limits, and
loader error classification.

## Consequences

There is one ZIP/DEFLATE implementation to harden and fuzz. Phase 1, Phase 2, and Phase 3 retain
independent domain resource limits and path policies without adding a crate dependency edge or a
third-party archive dependency. The root `graphene` facade does not re-export this codec; it is an
internal workspace primitive rather than a host-facing launcher API.
