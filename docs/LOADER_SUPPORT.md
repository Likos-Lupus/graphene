# Loader Support

This matrix is evidence-based. “Tier A fixture” means the normalized family is covered by local
fixture/source tests; it is **not** a claim that every release has real-smoke certification. Real
smoke status is owned by [`VALIDATION.md`](VALIDATION.md).

| Loader        | Exact fixture release           | Minecraft base | Profile / installer family                              | Support               | Fixture status                                                | Real smoke | Known limitations                                                                                                              |
|---------------|---------------------------------|----------------|---------------------------------------------------------|-----------------------|---------------------------------------------------------------|------------|--------------------------------------------------------------------------------------------------------------------------------|
| Fabric Loader | `0.16.14` profile-shape fixture | `1.21.1`       | Fabric Meta launcher profile                            | Tier A fixture        | Implemented; Cargo not executable here                        | NOT RUN    | Production pair is revalidated through Fabric Meta; Fabric API is not installed implicitly.                                    |
| Forge         | `fixture-loader`                | `1.21.1`       | modern `install_profile.json` processor profile, spec 1 | Tier A fixture        | Implemented; Cargo not executable here                        | NOT RUN    | Production list exposes official promoted latest/recommended; arbitrary exact pins validate official Maven installer/checksum. |
| Forge         | `fixture-legacy`                | `1.12.2`       | legacy launcher profile                                 | Tier B / MetadataOnly | Classification fixture implemented; Cargo not executable here | NOT RUN    | No legacy processor/install execution is claimed.                                                                              |
| NeoForge      | `fixture-loader`                | `1.21.1`       | modern Forge-family processor profile, spec 1           | Tier A fixture        | Implemented; Cargo not executable here                        | NOT RUN    | Version grammar remains adapter-local; base compatibility is rechecked from verified installer metadata.                       |

## Real-smoke pins

The maintained real-smoke procedures currently use:

- Fabric: Minecraft `1.21.1`, Fabric Loader `0.16.14`;
- Forge: Minecraft `1.21.1`, Forge `52.1.0`;
- NeoForge: Minecraft `1.21.1`, NeoForge `21.1.200`.

None has been executed in this repository environment; all remain `NOT RUN`.

## Fabric

Tier A normalization covers exact profile identity, main class, runtime libraries, JVM/game
arguments, provider integrity, component composition, and no-preparation installation. Selecting
Fabric Loader does not imply Fabric API/content installation.

## Forge

Modern normalization covers verified installer parsing, typed data, client processor filtering,
explicit executable/classpath inputs, allowlisted placeholders, embedded inputs, optional upstream
tool-Java requirements, declared generated outputs, verification/provenance, and shared output
reuse. Unknown profile specs or unsupported executable behavior fail closed.

Legacy launcher-profile Forge is detected explicitly and returns metadata-only support instead of
being treated as a fallback after failed modern parsing.

## NeoForge

NeoForge remains a separate discovery/version/integrity adapter and converges on the shared
Forge-family preparation recipe only after its verified installer has been parsed. The current
adapter prefers/requires SHA-256 sidecar integrity where available by its supported contract.
