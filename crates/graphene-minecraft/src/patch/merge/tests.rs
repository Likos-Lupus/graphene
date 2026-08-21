use super::*;
use crate::{
    Argument, ComponentProvenance, ComponentUid, ComponentVersion, Library, MavenCoordinate,
    MinecraftArch, MinecraftJavaRequirement, MinecraftOs, MinecraftVersionId, MinecraftVersionType,
    ResolvedComponent,
    test_support::{assets, context, resolved_artifact},
};
use std::collections::BTreeMap;

fn base() -> ResolvedMinecraft {
    ResolvedMinecraft {
        version_id: MinecraftVersionId::new("1.21.1").expect("version"),
        version_type: MinecraftVersionType::Release,
        main_class: "net.minecraft.client.main.Main".into(),
        client: resolved_artifact(1, ".minecraft/versions/1.21.1/1.21.1.jar"),
        libraries: Vec::new(),
        assets: assets(),
        logging: None,
        jvm_args: Vec::new(),
        game_args: Vec::new(),
        java_requirement: MinecraftJavaRequirement {
            major_version: 21,
            component_hint: None,
        },
        components: vec![ResolvedComponent::minecraft("1.21.1").expect("component")],
    }
}

fn loader_component() -> ResolvedComponent {
    ResolvedComponent {
        uid: ComponentUid::new("net.fabricmc.fabric-loader").expect("uid"),
        version: ComponentVersion::new("0.16.14").expect("version"),
        kind: crate::ComponentKind::Loader,
        provenance: ComponentProvenance::new("fabric", None).expect("provenance"),
    }
}

#[test]
fn scalar_and_arguments_compose_without_synthetic_base_version() {
    let mut patch = MinecraftVersionPatch::empty(loader_component());
    patch.main_class = Some("net.fabricmc.loader.impl.launch.knot.KnotClient".into());
    patch
        .jvm_args
        .push(Argument::Literal("-Dfabric=true".into()));
    let resolved = compose_minecraft(
        base(),
        &[patch],
        &context(MinecraftOs::Linux, MinecraftArch::X86_64),
    )
    .expect("compose");

    assert_eq!(resolved.version_id.as_str(), "1.21.1");
    assert_eq!(resolved.components.len(), 2);
    assert_eq!(
        resolved.jvm_args,
        vec![Argument::Literal("-Dfabric=true".into())]
    );
}

#[test]
fn library_replacement_uses_maven_identity() {
    let mut base = base();
    base.libraries.push(ResolvedLibrary {
        coordinate: MavenCoordinate::parse("org.example:demo:1").expect("coordinate"),
        classpath_artifact: Some(resolved_artifact(
            2,
            "shared/libraries/org/example/demo/1/demo-1.jar",
        )),
        native_artifact: None,
    });

    let mut patch = MinecraftVersionPatch::empty(loader_component());
    patch.libraries.push(Library {
        coordinate: MavenCoordinate::parse("org.example:demo:2").expect("coordinate"),
        rules: Vec::new(),
        artifact: Some(resolved_artifact(
            3,
            "shared/libraries/org/example/demo/2/demo-2.jar",
        )),
        classifiers: BTreeMap::new(),
        natives: BTreeMap::new(),
    });

    let resolved = compose_minecraft(
        base,
        &[patch],
        &context(MinecraftOs::Linux, MinecraftArch::X86_64),
    )
    .expect("compose");
    assert_eq!(resolved.libraries.len(), 1);
    assert_eq!(resolved.libraries[0].coordinate.version, "2");
}

#[test]
fn incompatible_java_requirement_fails_structurally() {
    let mut patch = MinecraftVersionPatch::empty(loader_component());
    patch.java_requirement = Some(MinecraftJavaRequirement {
        major_version: 17,
        component_hint: None,
    });

    assert_eq!(
        compose_minecraft(
            base(),
            &[patch],
            &context(MinecraftOs::Linux, MinecraftArch::X86_64)
        )
        .expect_err("conflict")
        .code,
        ErrorCode::ComponentJavaConflict
    );
}
