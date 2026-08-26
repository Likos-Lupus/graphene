//! Root facade workflow coverage for the modpack service.

use graphene::{Graphene, ModrinthProviderConfig, PackSource};
use graphene_core::archive::DeterministicZipWriter;
use std::io::Cursor;
use tempfile::tempdir;

/// A host can drive the modpack workflow end to end through the root facade without
/// touching provider DTOs, archive internals, or string parsing.
#[tokio::test]
async fn facade_inspects_a_graphene_style_pack_without_internal_knowledge() {
    let data_root = tempdir().expect("data root");
    let pack_root = tempdir().expect("pack root");

    let manifest = serde_json::json!({
        "schema_version": 1,
        "pack": { "name": "Facade Fixture", "version": "1.0.0" },
        "runtime": { "minecraft_version": "1.21.1" },
        "managed_files": [
            {
                "destination": "mods/remote.jar",
                "sha256": "11b7c8d2f4e5a6f7b8c9d0e1f2a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5",
                "size": 18,
                "sources": ["https://cdn.example.com/remote.jar"]
            }
        ],
        "seed_files": []
    });
    let mut writer = DeterministicZipWriter::new(Cursor::new(Vec::new()));
    writer
        .add_entry(
            "graphene.pack.json",
            serde_json::to_vec(&manifest).unwrap().as_slice(),
        )
        .expect("manifest entry");
    writer.finish().expect("finish");
    let pack_path = pack_root.path().join("facade.zip");
    std::fs::write(
        &pack_path,
        writer.into_inner().expect("finalized").into_inner(),
    )
    .expect("fixture written");

    let engine = Graphene::builder(data_root.path())
        .modrinth_provider(
            ModrinthProviderConfig::fixture("http://127.0.0.1:9").expect("fixture config"),
        )
        .build()
        .await
        .expect("engine builds");

    let inspection = engine
        .modpacks()
        .inspect(PackSource::LocalFile(pack_path))
        .await_result()
        .await
        .expect("inspection succeeds through the facade");

    assert_eq!(inspection.metadata().name(), "Facade Fixture");
}
