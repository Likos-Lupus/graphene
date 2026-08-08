use crate::*;
use graphene_core::{Artifact, ArtifactId, ArtifactIntegrity, ArtifactSource, Sha1Digest};
use std::collections::BTreeMap;

pub(crate) fn resolved_artifact(id_byte: u8, path: &str) -> ResolvedArtifact {
    let digest: Sha1Digest = "0000000000000000000000000000000000000000"
        .parse()
        .expect("fixture digest");
    let mut artifact = Artifact::new(
        vec![ArtifactSource::new("https://example.invalid/artifact")],
        ArtifactIntegrity::none().with_sha1(digest),
    );

    artifact.id = ArtifactId::from_bytes([id_byte; 16]);
    artifact.expected_size = Some(1);

    ResolvedArtifact {
        artifact,
        relative_path: ManagedPath::new(path).expect("fixture path"),
    }
}

pub(crate) fn assets() -> ResolvedAssets {
    ResolvedAssets {
        index_id: "fixture".into(),
        index: resolved_artifact(40, "shared/assets/indexes/fixture.json"),
        objects: Vec::new(),
        virtual_layout: false,
        map_to_resources: false,
    }
}

pub(crate) fn complete_metadata(libraries: Vec<Library>) -> MinecraftVersionMetadata {
    MinecraftVersionMetadata {
        id: MinecraftVersionId::new("1.21.1").expect("version"),
        version_type: MinecraftVersionType::Release,
        inherits_from: None,
        main_class: Some("net.minecraft.client.main.Main".into()),
        client: Some(resolved_artifact(
            1,
            ".minecraft/versions/1.21.1/1.21.1.jar",
        )),
        jvm_args: vec![Argument::Literal("-Xmx1G".into())],
        game_args: vec![Argument::Literal("--demo".into())],
        legacy_minecraft_arguments: None,
        libraries,
        asset_index: None,
        assets_id: Some("fixture".into()),
        logging: None,
        java_requirement: Some(MinecraftJavaRequirement {
            major_version: 21,
            component_hint: Some("java-runtime-gamma".into()),
        }),
        release_time: None,
        compliance_level: Some(1),
    }
}

pub(crate) fn context(os: MinecraftOs, arch: MinecraftArch) -> RuleContext {
    RuleContext {
        os,
        arch,
        os_version: Some("10.0.22631".into()),
        features: BTreeMap::new(),
    }
}
