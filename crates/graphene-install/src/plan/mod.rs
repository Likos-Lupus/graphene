mod build;
mod validate;

use graphene_core::{Artifact, ArtifactId};
use graphene_instance::{InstallReceipt, InstanceDescriptor};
use graphene_minecraft::{
    ComponentPreparationRecipe, ManagedPath, MinecraftVersionId, ResolvedArtifact,
    ResolvedMinecraft,
};
use serde::{Deserialize, Serialize};

pub const INSTALL_PLAN_VERSION: u32 = 2;
pub const MAX_INSTALL_ARTIFACTS: usize = 600_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedInstance {
    pub descriptor: InstanceDescriptor,
    pub relative_root: ManagedPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedArtifact {
    pub artifact: Artifact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MaterializationScope {
    Shared,
    Instance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Materialization {
    pub artifact_id: ArtifactId,
    pub destination: ManagedPath,
    pub scope: MaterializationScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeExtraction {
    pub artifact_id: ArtifactId,
    /// Relative to the instance root.
    pub destination: ManagedPath,
}

/// Complete deterministic installation intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallPlan {
    pub plan_version: u32,
    pub instance: PlannedInstance,
    pub requested_version: MinecraftVersionId,
    pub minecraft: ResolvedMinecraft,
    pub artifacts: Vec<PlannedArtifact>,
    pub shared_materializations: Vec<Materialization>,
    pub instance_materializations: Vec<Materialization>,
    pub native_extractions: Vec<NativeExtraction>,
    pub metadata_artifacts: Vec<ResolvedArtifact>,
    pub preparation: ComponentPreparationRecipe,
    pub receipt: InstallReceipt,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InstallRequest;
    use graphene_core::{ArtifactIntegrity, ArtifactSource, InstanceId, Sha1Digest};
    use graphene_instance::NewInstanceSpec;
    use graphene_minecraft::{
        Argument, MinecraftJavaRequirement, MinecraftVersionType, ResolvedAssets,
    };

    fn fixture_artifact(id: u8, path: &str) -> ResolvedArtifact {
        let digest = Sha1Digest::from_bytes([id; 20]);
        let mut artifact = Artifact::new(
            vec![ArtifactSource::new("https://example.invalid/artifact")],
            ArtifactIntegrity::none().with_sha1(digest),
        );

        artifact.id = ArtifactId::from_bytes([id; 16]);
        artifact.expected_size = Some(1);
        ResolvedArtifact {
            artifact,
            relative_path: ManagedPath::new(path).expect("path"),
        }
    }

    fn fixture_minecraft() -> ResolvedMinecraft {
        let index = fixture_artifact(2, "shared/assets/indexes/fixture.json");
        ResolvedMinecraft {
            version_id: MinecraftVersionId::new("fixture-1").expect("version"),
            version_type: MinecraftVersionType::Release,
            main_class: "example.Main".into(),
            client: fixture_artifact(1, ".minecraft/versions/fixture-1/fixture-1.jar"),
            libraries: Vec::new(),
            assets: ResolvedAssets {
                index_id: "fixture".into(),
                index,
                objects: Vec::new(),
                virtual_layout: false,
                map_to_resources: false,
            },
            logging: None,
            jvm_args: vec![Argument::Literal("-Dfixture=true".into())],
            game_args: Vec::new(),
            java_requirement: MinecraftJavaRequirement {
                major_version: 21,
                component_hint: None,
            },
            components: vec![
                graphene_minecraft::ResolvedComponent::minecraft("fixture-1").expect("component"),
            ],
        }
    }

    #[test]
    fn deterministic_plan_enumerates_client_and_asset_index() {
        let request = InstallRequest::with_instance(
            NewInstanceSpec::with_id(InstanceId::from_bytes([9; 16]), "Fixture").expect("instance"),
            MinecraftVersionId::new("fixture-1").expect("version"),
        );
        let plan = InstallPlan::build(request, fixture_minecraft(), Vec::new()).expect("plan");

        assert_eq!(plan.artifacts.len(), 2);
        assert_eq!(plan.instance_materializations.len(), 1);
        assert_eq!(plan.shared_materializations.len(), 1);
        plan.validate().expect("valid plan");
    }

    #[test]
    fn materialization_paths_cannot_cross_scope() {
        let materialization = Materialization {
            artifact_id: ArtifactId::from_bytes([1; 16]),
            destination: ManagedPath::new("shared/file").expect("path"),
            scope: MaterializationScope::Instance,
        };
        assert!(validate::validate_materialization_path(&materialization).is_err());
    }
}
