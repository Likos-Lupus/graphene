# Phase 3 Loader Support Matrix

This matrix is intentionally evidence-based. “Tier A fixture” means the normalized family is covered
by deterministic local fixture/source tests in this implementation candidate; it is **not** a claim
that every release in that loader family has been real-smoke certified. Real smoke status is
recorded separately.

| Loader        | Exact pinned fixture release    | Minecraft base | Profile / installer family                              | Support tier          | Tool Java in fixture              | Fixture status                                              | Real smoke | Known limitations                                                                                                                                                                                 |
|---------------|---------------------------------|----------------|---------------------------------------------------------|-----------------------|-----------------------------------|-------------------------------------------------------------|------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Fabric Loader | `0.16.14` profile-shape fixture | `1.21.1`       | Fabric Meta launcher profile                            | Tier A fixture        | none                              | implemented, not Cargo-executed here                        | NOT RUN    | Production pair is revalidated through Fabric Meta; Fabric API is not installed; fixture success is not a real launch certification.                                                              |
| Forge         | `fixture-loader`                | `1.21.1`       | modern `install_profile.json` processor profile, spec 1 | Tier A fixture        | not declared by installer fixture | implemented, not Cargo-executed here                        | NOT RUN    | Production version listing exposes official promoted latest/recommended only; arbitrary exact pins are checked via official Maven installer/checksum; legacy launcher profiles are metadata-only. |
| Forge         | `fixture-legacy`                | `1.12.2`       | legacy launcher profile                                 | Tier B / MetadataOnly | n/a                               | classification fixture implemented, not Cargo-executed here | NOT RUN    | No legacy processor/install execution is claimed.                                                                                                                                                 |
| NeoForge      | `fixture-loader`                | `1.21.1`       | modern Forge-family processor profile, spec 1           | Tier A fixture        | not declared by installer fixture | implemented, not Cargo-executed here                        | NOT RUN    | Version grammar remains adapter-local and authoritative base compatibility is checked again from the verified installer profile.                                                                  |

## Production Smoke Pins

The real smoke documents pin stable/reproducible upstream identities independently from the
synthetic normalization fixtures:

- Fabric: Minecraft `1.21.1`, Fabric Loader `0.16.14`;
- Forge: Minecraft `1.21.1`, Forge `52.1.0` (official recommended pair at documentation time);
- NeoForge: Minecraft `1.21.1`, NeoForge `21.1.200`.

None of these real-smoke pins was executed in this session. The corresponding smoke documents remain
`Status: NOT RUN`.

## Capability Notes

### Fabric

Tier A normalization covers exact profile identity, main class, runtime libraries, JVM/game
arguments, provider integrity, component graph composition, and no-preparation installation. A
Fabric profile never implies Fabric API/content installation.

### Forge

Modern processor-profile normalization covers verified installer parsing, typed data variables,
client-side processor filtering, explicit executable/classpath inputs, allowed placeholders,
embedded inputs, optional tool Java requirements when upstream declares them, declared generated
outputs, hash verification/provenance, and shared generated-output reuse. The fixture version
metadata requests Java 21 for the game; that is not treated as an installer-tool Java declaration.
Unknown spec versions or unsupported executable behavior fail closed.

Legacy Forge is intentionally not advertised as Tier A. The provider detects the family explicitly
and returns metadata-only support instead of treating a failed modern parse as legacy.

### NeoForge

NeoForge remains a separate discovery/version/integrity adapter and converges on the generic
Forge-family preparation recipe only after its verified installer has been parsed. SHA-256 sidecars
are preferred/required by the current adapter.
