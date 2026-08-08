# Phase 1 deterministic fixtures

These fixtures are synthetic, frozen, official-compatible shapes for the pinned Phase 1 acceptance
identity `1.21.1`. They are not redistributed Mojang binaries. Provider URLs inside frozen metadata
use `https://fixture.invalid` and are rewritten only by explicit fixture configuration to the local
deterministic HTTP test server. Normal CI must not contact public Mojang infrastructure.

The fixture set contains the normal pinned metadata/version/asset path, correctly hashed malformed
metadata, a correctly hashed version that selects a malicious traversal native archive, immutable
client/library/asset/logging bodies, bounded native ZIP/JAR cases (valid deflate, parent traversal,
absolute path, and symlink-shaped entry), and the cross-platform fake Java source. The fake helper
is compiled by integration tests with the same Rust toolchain used to run the workspace, avoiding
any dependency on a system Java installation.

## File digests

| Path                                                | Bytes | SHA-1                                      | SHA-256                                                            |
|-----------------------------------------------------|------:|--------------------------------------------|--------------------------------------------------------------------|
| `artifacts/client.jar`                              |    35 | `ccadb31bf117f00a9c5658ce2042871a47848140` | `45df54cb7934f3642d875b60f2c0d475233e0f6b21a1712e3500cbe578782f76` |
| `artifacts/fixture-lib-1.0.jar`                     |    32 | `aa386b701a012679e5ff9ab7690205adb2d9fb20` | `47b65e205ef60344353dd3c27010180093b7521d3ac04003b27219d59cb6fabf` |
| `artifacts/fixture.ogg`                             |    32 | `2e33cf671c7dfe5f545e01ee202ad3caf8e55da1` | `e254a7b557461b56b6ef3accac8d2530b3c67e6d25ffcb4345111a0ea9ee4323` |
| `artifacts/log4j2.xml`                              |    58 | `d97bf6f9023937d38492b458bfc26ced6d727065` | `a02aa22e0eb3487891205a7d2c2d3893f1b3d2b746b6dda36c51beb2194f320d` |
| `java/fake_java.rs`                                 |  2105 | `570f1cdadb57961673b252ecc0c042229290115e` | `4aaabf241cf798436ba61136bd8177d15ccbbad2825fcdd005486b070c56a39d` |
| `minecraft/1.21.1-assets.json`                      |   148 | `d09d566d9ad5fbbc83841a4b65bcbca21eaf9c16` | `f000e69d02401011d204a8d910b15dda6bc6c7e83821f65da505c6f622846907` |
| `minecraft/1.21.1-traversal.json`                   |  2687 | `b488988e1fe0460d188a92d38ee122f8058796eb` | `43c124561824334ef5536fbf56fa6cd5f6b226ba5c8c53471ddcf2d1d39a6aaf` |
| `minecraft/1.21.1.json`                             |  2702 | `ba104db5930e73ee228c6a9c8673d4d56d24d562` | `d1df3992dd7cb05e0d7cd4ad8643f188208ca3016660186fa37428d208bc2900` |
| `minecraft/malformed-version.json`                  |     1 | `60ba4b2daa4ed4d070fec06687e249e0e6f9ee45` | `021fb596db81e6d02bf3d2586ee3981fe519f275c0ac9ca76bbcf2ebb4097d96` |
| `minecraft/version_manifest_malformed_version.json` |   222 | `959b8b49a86809a14e4f64ed1c7e523aaa1ba886` | `5654f7f267501a6fa7764cfd6918f2f6f7bd72da20780d4c41b8a29c40b36acb` |
| `minecraft/version_manifest_traversal.json`         |   281 | `70c5dd057671f0f7c36057c6de55f5fe33828c6f` | `0dd869d586506e4f935d50952c9fb84baffd4166f6783fa1f7d83a898968afa3` |
| `minecraft/version_manifest_v2.json`                |   271 | `4372a27ccddc0e80cace20275ecaa435362cb317` | `45ba82de9855aa0dbf3725a06071b9ec9c4bd50fa9192a94b423d61b3a9350c8` |
| `native-jars/absolute.jar`                          |   130 | `0db0dc8b3c78c99f73fff86ad49cf0457d1df0dc` | `5c9aa8f7ac9b9b000e4410c9921604ca8ccae15b6e42f9b082f145e7e39ee998` |
| `native-jars/fixture-native.jar`                    |   319 | `426bdb2a28be39a753c3694b80c530a1320e3b5b` | `4b4233454f2a415a3c6a89b7f3667a6361add3396a106ce124d9f0ef1c10940a` |
| `native-jars/symlink.jar`                           |   115 | `2c659c6149a0854d46a222ecec0116a3c59e60a9` | `2513df495787f704d294574a2e301f4356243d20af0bae4074d3b0ee0e629f69` |
| `native-jars/traversal.jar`                         |   130 | `1a147aceadb82dad19ff1cae8cf9afc837098a16` | `a0d907fca02f00e63e47672389a057e2006f53e19b41966580706904fb44441e` |

## Covered behavior

The fixtures cover manifest/version/asset normalization, explicit pinned version selection, modern
argument/rule normalization, Java requirement normalization, verified metadata/artifact acquisition,
cache/shared reuse, complete install planning, native selection/extraction, archive attack
rejection, provider-neutral receipt persistence, offline launch reconstruction,
deterministic/redacted snapshots, Java 8/modern probe parsing paths, fake process
stdout/stderr/PID/exit/kill/backpressure, and secret delivery/redaction.
