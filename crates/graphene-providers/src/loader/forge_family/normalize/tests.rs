use super::*;
use graphene_core::{ArtifactIntegrity, OperationRegistry};
use graphene_minecraft::{
    ComponentConflict, ComponentDescriptor, ComponentKind, ComponentProvenance,
    ComponentRequirement, ComponentUid, ComponentVersion, LoaderVersion, MinecraftVersionId,
    MinecraftVersionPatch, ResolvedComponent,
};
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/loaders")
        .join(name)
}

fn shell(kind: LoaderKind, minecraft: &str) -> ResolvedLoader {
    let uid = ComponentUid::new(kind.component_uid()).expect("component uid");
    let version = LoaderVersion::new("fixture-loader").expect("loader version");
    let component_version = ComponentVersion::new(version.as_str()).expect("component version");
    let descriptor = ComponentDescriptor {
        uid: uid.clone(),
        version: component_version.clone(),
        kind: ComponentKind::Loader,
        order: 100,
        requires: vec![ComponentRequirement::Exact {
            uid: ComponentUid::new("net.minecraft").expect("minecraft uid"),
            version: ComponentVersion::new(minecraft).expect("minecraft version"),
        }],
        conflicts: [LoaderKind::Fabric, LoaderKind::Forge, LoaderKind::NeoForge]
            .into_iter()
            .filter(|candidate| *candidate != kind)
            .map(|candidate| ComponentConflict {
                uid: ComponentUid::new(candidate.component_uid()).expect("conflict uid"),
            })
            .collect(),
    };
    let component = ResolvedComponent {
        uid,
        version: component_version,
        kind: ComponentKind::Loader,
        provenance: ComponentProvenance::new(kind.provider_id(), Some("fixture".to_owned()))
            .expect("provenance"),
    };
    let integrity = ArtifactIntegrity::none().with_sha1(
        "d83ef5c6f32a695513e248ff8eee0996c1b31125"
            .parse()
            .expect("sha1"),
    );
    let installer = ResolvedArtifact {
        artifact: artifact(
            "https://example.invalid/installer.jar".to_owned(),
            integrity,
            None,
            false,
        )
        .expect("installer"),
        relative_path: ManagedPath::new("shared/loader-installers/fixture/installer.jar")
            .expect("managed path"),
    };
    ResolvedLoader {
        kind,
        version,
        minecraft: MinecraftVersionId::new(minecraft).expect("minecraft"),
        component: descriptor,
        patch: MinecraftVersionPatch::empty(component.clone()),
        preparation: ComponentPreparationRecipe {
            component: Some(component),
            installer: Some(installer),
            ..Default::default()
        },
        support: LoaderSupport::MetadataOnly {
            reason: "fixture shell".to_owned(),
        },
    }
}

fn normalize(name: &str, family: ForgeFamily) -> Result<ResolvedLoader> {
    let registry = OperationRegistry::new(16).expect("operation registry");
    let operation = registry.create("normalize-fixture");
    normalize_verified_installer(
        shell(family.kind(), "1.21.1"),
        &fixture(name),
        &operation,
        family,
        "https://maven.example.invalid",
        false,
    )
}

#[test]
fn modern_forge_profile_normalizes_into_patch_and_recipe() {
    let resolved = normalize("forge-modern-fixture.jar", ForgeFamily::Forge).expect("normalize");
    assert_eq!(resolved.support, LoaderSupport::Supported);
    assert_eq!(
        resolved.patch.main_class.as_deref(),
        Some("net.minecraftforge.bootstrap.ForgeBootstrap")
    );
    assert_eq!(
        resolved.preparation.processors.len(),
        1,
        "server-only processor is skipped"
    );
    assert_eq!(resolved.preparation.generated_outputs.len(), 1);
    assert!(
        resolved.preparation.generated_outputs[0]
            .expected_integrity
            .is_verifiable()
    );
    assert!(
        !resolved.preparation.generated_outputs[0]
            .input_identity
            .is_empty()
    );
    assert_eq!(
        resolved
            .patch
            .java_requirement
            .as_ref()
            .map(|value| value.major_version),
        Some(21)
    );
}

#[test]
fn neoforge_converges_on_same_preparation_model() {
    let resolved = normalize("neoforge-modern-fixture.jar", ForgeFamily::NeoForge)
        .expect("normalize NeoForge");
    assert_eq!(resolved.kind, LoaderKind::NeoForge);
    assert_eq!(resolved.support, LoaderSupport::Supported);
    assert_eq!(resolved.preparation.processors.len(), 1);
    assert_eq!(resolved.preparation.generated_outputs.len(), 1);
}

#[test]
fn embedded_client_input_is_declared_with_consumers() {
    let resolved = normalize("forge-modern-embedded-fixture.jar", ForgeFamily::Forge)
        .expect("normalize embedded fixture");
    let embedded = resolved
        .preparation
        .embedded_inputs
        .iter()
        .find(|entry| entry.entry == "data/client.lzma")
        .expect("client embedded input");
    assert!(!embedded.consumers.is_empty());
    assert!(
        resolved
            .preparation
            .embedded_inputs
            .iter()
            .all(|entry| entry.entry != "data/server.lzma"),
        "server-only data must not enter the client recipe"
    );
}

#[test]
fn legacy_profile_is_metadata_only_not_modern_fallback() {
    let registry = OperationRegistry::new(16).expect("operation registry");
    let operation = registry.create("legacy-fixture");
    let resolved = normalize_verified_installer(
        shell(LoaderKind::Forge, "1.12.2"),
        &fixture("forge-legacy-fixture.jar"),
        &operation,
        ForgeFamily::Forge,
        "https://maven.example.invalid",
        false,
    )
    .expect("legacy classification");
    assert!(matches!(
        resolved.support,
        LoaderSupport::MetadataOnly { .. }
    ));
    assert!(resolved.preparation.processors.is_empty());
}

#[test]
fn hostile_installer_fixtures_fail_before_execution() {
    for name in [
        "hostile/traversal.jar",
        "hostile/absolute.jar",
        "hostile/malformed-json.jar",
        "hostile/duplicate-profile.jar",
        "hostile/unknown-placeholder.jar",
        "hostile/output-escape.jar",
        "hostile/nul-argument.jar",
        "hostile/script-reference.jar",
        "hostile/unsafe-repository.jar",
        "hostile/unsupported-spec.jar",
        "hostile/base-mismatch.jar",
        "hostile/oversized-metadata.jar",
        "hostile/oversized-embedded.jar",
        "hostile/too-many-entries.jar",
        "hostile/symlink-entry.jar",
        "hostile/special-file.jar",
    ] {
        assert!(fixture(name).is_file(), "missing hostile fixture: {name}");
        assert!(
            normalize(name, ForgeFamily::Forge).is_err(),
            "{name} must fail"
        );
    }
}
