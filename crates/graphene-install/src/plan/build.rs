use super::validate::validate_materialization_path;
use super::{
    INSTALL_PLAN_VERSION, InstallPlan, MAX_INSTALL_ARTIFACTS, Materialization,
    MaterializationScope, NativeExtraction, PlannedArtifact, PlannedInstance,
};
use crate::{
    error::install_error,
    request::{ComponentInstallRequest, InstallRequest},
};
use graphene_core::{ArtifactId, ErrorCode, Result};
use graphene_instance::{
    INSTALL_FORMAT_VERSION, INSTALL_RECEIPT_SCHEMA_VERSION, InstallReceipt, InstalledArgument,
    InstalledArtifact, InstalledComponent, InstalledComponentKind, InstalledJavaRequirement,
    InstalledLibrary, InstalledRule, InstalledRuleAction, InstanceDescriptor,
    ManagedRelativePath as ReceiptPath,
};
use graphene_minecraft::{
    Argument, ComponentPreparationRecipe, ManagedPath, MavenCoordinate, MinecraftArch, MinecraftOs,
    MinecraftVersionType, ResolvedArtifact, ResolvedLibrary, ResolvedMinecraft, Rule, RuleAction,
};
use std::collections::{HashMap, HashSet};

impl InstallPlan {
    /// Builds the full plan from normalized provider-neutral metadata before committed mutation.
    pub fn build(
        request: InstallRequest,
        minecraft: ResolvedMinecraft,
        metadata_artifacts: Vec<ResolvedArtifact>,
    ) -> Result<Self> {
        Self::build_with_preparation(
            request,
            minecraft,
            metadata_artifacts,
            ComponentPreparationRecipe::default(),
        )
    }

    pub fn build_component(
        request: ComponentInstallRequest,
        minecraft: ResolvedMinecraft,
        metadata_artifacts: Vec<ResolvedArtifact>,
        preparation: ComponentPreparationRecipe,
    ) -> Result<Self> {
        let vanilla = InstallRequest::with_instance(request.instance, request.minecraft_version);
        Self::build_with_preparation(vanilla, minecraft, metadata_artifacts, preparation)
    }

    fn build_with_preparation(
        request: InstallRequest,
        minecraft: ResolvedMinecraft,
        metadata_artifacts: Vec<ResolvedArtifact>,
        preparation: ComponentPreparationRecipe,
    ) -> Result<Self> {
        if request.minecraft_version != minecraft.version_id {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "resolved Minecraft version does not match the install request",
            ));
        }

        let descriptor = InstanceDescriptor::create(&request.instance);
        let instance_root = ManagedPath::new(format!("instances/{}", request.instance.id))?;
        let natives_directory = ManagedPath::new(format!(
            ".graphene/natives/{}",
            minecraft.version_id.as_str()
        ))?;

        let mut artifacts = Vec::<PlannedArtifact>::new();
        let mut shared = Vec::<Materialization>::new();
        let mut instance = Vec::<Materialization>::new();
        let mut natives = Vec::<NativeExtraction>::new();
        let mut artifact_ids = HashSet::new();
        let mut destinations = HashMap::<String, ArtifactId>::new();

        let generated_destinations = preparation
            .generated_outputs
            .iter()
            .filter(|output| {
                matches!(
                    output.scope,
                    graphene_minecraft::GeneratedOutputScope::SharedImmutable
                )
            })
            .map(|output| output.managed_destination.as_str().to_owned())
            .collect::<HashSet<_>>();

        add_materialization(
            &minecraft.client,
            MaterializationScope::Instance,
            &mut artifacts,
            &mut instance,
            &mut artifact_ids,
            &mut destinations,
        )?;

        for library in &minecraft.libraries {
            if let Some(classpath) = &library.classpath_artifact
                && !generated_destinations.contains(classpath.relative_path.as_str())
            {
                add_materialization(
                    classpath,
                    MaterializationScope::Shared,
                    &mut artifacts,
                    &mut shared,
                    &mut artifact_ids,
                    &mut destinations,
                )?;
            }

            if let Some(native) = &library.native_artifact {
                add_materialization(
                    native,
                    MaterializationScope::Shared,
                    &mut artifacts,
                    &mut shared,
                    &mut artifact_ids,
                    &mut destinations,
                )?;
                natives.push(NativeExtraction {
                    artifact_id: native.artifact.id,
                    destination: natives_directory.clone(),
                });
            }
        }

        add_materialization(
            &minecraft.assets.index,
            MaterializationScope::Shared,
            &mut artifacts,
            &mut shared,
            &mut artifact_ids,
            &mut destinations,
        )?;

        for object in &minecraft.assets.objects {
            add_materialization(
                &object.artifact,
                MaterializationScope::Shared,
                &mut artifacts,
                &mut shared,
                &mut artifact_ids,
                &mut destinations,
            )?;
        }

        if let Some(logging) = &minecraft.logging {
            add_materialization(
                &logging.artifact,
                MaterializationScope::Shared,
                &mut artifacts,
                &mut shared,
                &mut artifact_ids,
                &mut destinations,
            )?;
        }

        for metadata in &metadata_artifacts {
            add_materialization(
                metadata,
                MaterializationScope::Shared,
                &mut artifacts,
                &mut shared,
                &mut artifact_ids,
                &mut destinations,
            )?;
        }

        if let Some(installer) = &preparation.installer {
            add_preparation_artifact(installer, &mut artifacts, &mut artifact_ids)?;
        }

        for input in &preparation.input_artifacts {
            add_preparation_artifact(input, &mut artifacts, &mut artifact_ids)?;
        }

        for processor in &preparation.processors {
            add_preparation_artifact(&processor.executable_jar, &mut artifacts, &mut artifact_ids)?;
            for classpath in &processor.classpath {
                add_preparation_artifact(classpath, &mut artifacts, &mut artifact_ids)?;
            }
        }

        if artifacts.len() > MAX_INSTALL_ARTIFACTS {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "installation plan contains too many artifacts",
            ));
        }

        natives.sort_by_key(|entry| entry.artifact_id.to_string());
        natives.dedup_by_key(|entry| entry.artifact_id);

        let receipt = InstallReceipt {
            schema_version: INSTALL_RECEIPT_SCHEMA_VERSION,
            install_format_version: INSTALL_FORMAT_VERSION,
            instance_id: request.instance.id,
            requested_version: request.minecraft_version.as_str().to_owned(),
            resolved_version: minecraft.version_id.as_str().to_owned(),
            components: minecraft
                .components
                .iter()
                .map(installed_component)
                .collect(),
            version_type: version_type_name(&minecraft.version_type),
            main_class: minecraft.main_class.clone(),
            java_requirement: InstalledJavaRequirement {
                major_version: minecraft.java_requirement.major_version,
                component_hint: minecraft.java_requirement.component_hint.clone(),
            },
            client: installed_artifact(&minecraft.client)?,
            libraries: minecraft
                .libraries
                .iter()
                .map(installed_library)
                .collect::<Result<Vec<_>>>()?,
            asset_index_id: minecraft.assets.index_id.clone(),
            asset_index: installed_artifact(&minecraft.assets.index)?,
            assets_root: ReceiptPath::new("shared/assets")?,
            natives_directory: ReceiptPath::new(natives_directory.as_str())?,
            logging_configuration: minecraft
                .logging
                .as_ref()
                .map(|entry| installed_artifact(&entry.artifact))
                .transpose()?,
            logging_argument: minecraft
                .logging
                .as_ref()
                .map(|entry| entry.argument.clone()),
            jvm_arguments: minecraft
                .jvm_args
                .iter()
                .map(installed_argument)
                .collect::<Result<Vec<_>>>()?,
            game_arguments: minecraft
                .game_args
                .iter()
                .map(installed_argument)
                .collect::<Result<Vec<_>>>()?,
        };
        receipt.validate().map_err(|source| {
            install_error(ErrorCode::InstallPlanInvalid, "install receipt is invalid")
                .with_source(source)
        })?;

        let plan = Self {
            plan_version: INSTALL_PLAN_VERSION,
            instance: PlannedInstance {
                descriptor,
                relative_root: instance_root,
            },
            requested_version: request.minecraft_version,
            minecraft,
            artifacts,
            shared_materializations: shared,
            instance_materializations: instance,
            native_extractions: natives,
            metadata_artifacts,
            preparation,
            receipt,
        };

        plan.validate()?;
        Ok(plan)
    }
}

fn add_preparation_artifact(
    value: &ResolvedArtifact,
    artifacts: &mut Vec<PlannedArtifact>,
    artifact_ids: &mut HashSet<ArtifactId>,
) -> Result<()> {
    if artifact_ids.insert(value.artifact.id) {
        artifacts.push(PlannedArtifact {
            artifact: value.artifact.clone(),
        });
    }
    Ok(())
}

fn installed_component(value: &graphene_minecraft::ResolvedComponent) -> InstalledComponent {
    InstalledComponent {
        uid: value.uid.as_str().to_owned(),
        version: value.version.as_str().to_owned(),
        kind: match value.kind {
            graphene_minecraft::ComponentKind::Minecraft => InstalledComponentKind::Minecraft,
            graphene_minecraft::ComponentKind::Loader => InstalledComponentKind::Loader,
            graphene_minecraft::ComponentKind::Auxiliary => InstalledComponentKind::Auxiliary,
            _ => InstalledComponentKind::Auxiliary,
        },
        provider: value.provenance.provider.clone(),
        provenance: value.provenance.detail.clone(),
    }
}

fn installed_artifact(value: &ResolvedArtifact) -> Result<InstalledArtifact> {
    Ok(InstalledArtifact {
        path: ReceiptPath::new(value.relative_path.as_str())?,
        integrity: value.artifact.integrity.clone(),
        expected_size: value.artifact.expected_size,
    })
}

fn installed_library(value: &ResolvedLibrary) -> Result<InstalledLibrary> {
    Ok(InstalledLibrary {
        coordinate: maven_coordinate_string(&value.coordinate),
        classpath: value
            .classpath_artifact
            .as_ref()
            .map(installed_artifact)
            .transpose()?,
        native_archive: value
            .native_artifact
            .as_ref()
            .map(installed_artifact)
            .transpose()?,
    })
}

fn maven_coordinate_string(value: &MavenCoordinate) -> String {
    let mut rendered = format!("{}:{}:{}", value.group, value.artifact, value.version);

    if let Some(classifier) = &value.classifier {
        rendered.push(':');
        rendered.push_str(classifier);
    }

    if value.extension != "jar" {
        rendered.push('@');
        rendered.push_str(&value.extension);
    }
    rendered
}

fn installed_argument(value: &Argument) -> Result<InstalledArgument> {
    Ok(match value {
        Argument::Literal(value) => InstalledArgument::Literal(value.clone()),
        Argument::Conditional { rules, values } => InstalledArgument::Conditional {
            rules: rules.iter().map(installed_rule).collect(),
            values: values.clone(),
        },
        _ => {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "Minecraft argument variant is unsupported by the install receipt",
            ));
        }
    })
}

fn installed_rule(value: &Rule) -> InstalledRule {
    InstalledRule {
        action: match value.action {
            RuleAction::Allow => InstalledRuleAction::Allow,
            RuleAction::Disallow => InstalledRuleAction::Disallow,
            _ => InstalledRuleAction::Disallow,
        },
        os_name: value
            .os
            .as_ref()
            .and_then(|os| os.name.as_ref())
            .map(os_name),
        os_architecture: value
            .os
            .as_ref()
            .and_then(|os| os.architecture.as_ref())
            .map(arch_name),
        os_version_pattern: value.os.as_ref().and_then(|os| os.version_pattern.clone()),
        features: value.features.clone(),
    }
}

fn os_name(value: &MinecraftOs) -> String {
    match value {
        MinecraftOs::Windows => "windows".to_owned(),
        MinecraftOs::Linux => "linux".to_owned(),
        MinecraftOs::Osx => "osx".to_owned(),
        MinecraftOs::Other(value) => value.clone(),
        _ => "unsupported".to_owned(),
    }
}

fn arch_name(value: &MinecraftArch) -> String {
    match value {
        MinecraftArch::X86 => "x86".to_owned(),
        MinecraftArch::X86_64 => "x86_64".to_owned(),
        MinecraftArch::AArch64 => "aarch64".to_owned(),
        MinecraftArch::Other(value) => value.clone(),
        _ => "unsupported".to_owned(),
    }
}

fn version_type_name(value: &MinecraftVersionType) -> String {
    match value {
        MinecraftVersionType::Release => "release".to_owned(),
        MinecraftVersionType::Snapshot => "snapshot".to_owned(),
        MinecraftVersionType::OldBeta => "old_beta".to_owned(),
        MinecraftVersionType::OldAlpha => "old_alpha".to_owned(),
        MinecraftVersionType::Other(value) => value.clone(),
        _ => "unsupported".to_owned(),
    }
}

fn add_materialization(
    resolved: &ResolvedArtifact,
    scope: MaterializationScope,
    artifacts: &mut Vec<PlannedArtifact>,
    materializations: &mut Vec<Materialization>,
    artifact_ids: &mut HashSet<ArtifactId>,
    destinations: &mut HashMap<String, ArtifactId>,
) -> Result<()> {
    let destination = resolved.relative_path.as_str().to_owned();
    if let Some(existing) = destinations.get(&destination) {
        if *existing != resolved.artifact.id {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "conflicting materialization destination",
            ));
        }

        return Ok(());
    }

    let materialization = Materialization {
        artifact_id: resolved.artifact.id,
        destination: resolved.relative_path.clone(),
        scope,
    };

    validate_materialization_path(&materialization)?;
    destinations.insert(destination, resolved.artifact.id);
    materializations.push(materialization);
    if artifact_ids.insert(resolved.artifact.id) {
        artifacts.push(PlannedArtifact {
            artifact: resolved.artifact.clone(),
        });
    }

    Ok(())
}
