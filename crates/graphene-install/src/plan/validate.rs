use super::{INSTALL_PLAN_VERSION, InstallPlan, Materialization, MaterializationScope};
use crate::{error::install_error, path::minecraft_path_to_platform};
use graphene_core::{Artifact, ArtifactId, ErrorCode, Result};
use graphene_minecraft::ResolvedArtifact;
use std::collections::{HashMap, HashSet};

impl InstallPlan {
    pub fn validate(&self) -> Result<()> {
        if self.plan_version != INSTALL_PLAN_VERSION {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "install plan version is unsupported",
            ));
        }

        self.instance.descriptor.validate().map_err(|source| {
            install_error(
                ErrorCode::InstallPlanInvalid,
                "planned instance descriptor is invalid",
            )
            .with_source(source)
        })?;
        self.receipt.validate().map_err(|source| {
            install_error(
                ErrorCode::InstallPlanInvalid,
                "planned install receipt is invalid",
            )
            .with_source(source)
        })?;

        if self.receipt.instance_id != self.instance.descriptor.instance_id
            || self.receipt.requested_version != self.requested_version.as_str()
            || self.receipt.resolved_version != self.minecraft.version_id.as_str()
            || !receipt_components_match(self)
            || self.receipt.client.path.as_str() != self.minecraft.client.relative_path.as_str()
        {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "install receipt does not match the plan",
            ));
        }

        if self.minecraft.main_class.trim().is_empty() || self.artifacts.is_empty() {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "install plan is incomplete",
            ));
        }

        let expected_root = format!("instances/{}", self.instance.descriptor.instance_id);
        if self.instance.relative_root.as_str() != expected_root {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "instance target does not match its ID",
            ));
        }

        let mut ids = HashMap::<ArtifactId, &Artifact>::new();
        for planned in &self.artifacts {
            // Source-less artifacts are valid only when a trustworthy content-addressed cache
            // identity exists (pack snapshots / embedded managed files acquired during planning).
            let cache_only_verifiable = planned.artifact.sources.is_empty()
                && (planned.artifact.integrity.sha256().is_some()
                    || planned.artifact.integrity.sha512().is_some());
            if (!cache_only_verifiable && planned.artifact.sources.is_empty())
                || !planned.artifact.integrity.is_verifiable()
            {
                return Err(install_error(
                    ErrorCode::InstallPlanInvalid,
                    "planned artifact is not verifiably acquirable",
                )
                .with_context("artifact_id", planned.artifact.id.to_string()));
            }

            if let Some(existing) = ids.insert(planned.artifact.id, &planned.artifact)
                && existing != &planned.artifact
            {
                return Err(install_error(
                    ErrorCode::InstallPlanInvalid,
                    "artifact ID is reused for conflicting artifacts",
                ));
            }
        }

        let mut paths = HashMap::<String, ArtifactId>::new();
        for materialization in self
            .shared_materializations
            .iter()
            .chain(&self.instance_materializations)
        {
            if !ids.contains_key(&materialization.artifact_id) {
                return Err(install_error(
                    ErrorCode::InstallPlanInvalid,
                    "materialization references an unknown artifact",
                ));
            }

            validate_materialization_path(materialization)?;
            if let Some(existing) = paths.insert(
                materialization.destination.as_str().to_owned(),
                materialization.artifact_id,
            ) && existing != materialization.artifact_id
            {
                return Err(install_error(
                    ErrorCode::InstallPlanInvalid,
                    "multiple artifacts target the same destination",
                ));
            }
        }

        for extraction in &self.native_extractions {
            if !ids.contains_key(&extraction.artifact_id)
                || !extraction
                    .destination
                    .as_str()
                    .starts_with(".graphene/natives/")
            {
                return Err(install_error(
                    ErrorCode::InstallPlanInvalid,
                    "native extraction target is invalid",
                ));
            }

            minecraft_path_to_platform(&extraction.destination)?;
        }

        validate_preparation(self, &ids)?;
        validate_plan_materialization_completeness(self)?;
        validate_seed_payload(self, &ids)?;

        Ok(())
    }
}

fn validate_seed_payload(plan: &InstallPlan, ids: &HashMap<ArtifactId, &Artifact>) -> Result<()> {
    use super::MAX_SEED_ARCHIVE_LAYERS;

    if plan.seed_archive_layers.len() > MAX_SEED_ARCHIVE_LAYERS {
        return Err(install_error(
            ErrorCode::InstallPlanInvalid,
            "seed archive layer count exceeds its bound",
        ));
    }

    let mut materialized_destinations = HashSet::new();
    for materialization in plan
        .shared_materializations
        .iter()
        .chain(&plan.instance_materializations)
    {
        materialized_destinations.insert(materialization.destination.as_str().to_ascii_lowercase());
    }

    let mut content_keys = HashSet::new();
    for entry in &plan.initial_content {
        if entry.entry_id.is_empty()
            || !content_keys.insert(entry.entry_id.as_str())
            || entry.artifact_logical_key.is_empty()
        {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "initial content entry identity is invalid or duplicated",
            ));
        }
    }
    // Every initial content entry must reference a planned managed artifact logical key.
    for entry in &plan.initial_content {
        let referenced = plan
            .instance_materializations
            .iter()
            .any(|materialization| {
                format!("instance:{}", materialization.destination.as_str())
                    == entry.artifact_logical_key
            });
        if !referenced {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "initial content entry references an unknown managed artifact",
            )
            .with_context("logical_key", entry.artifact_logical_key.clone()));
        }
    }
    drop(content_keys);

    for layer in &plan.seed_archive_layers {
        if !ids.contains_key(&layer.archive_artifact_id) {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "seed archive layer references an unknown artifact",
            ));
        }

        let destination = layer.destination.as_str();
        if !destination.starts_with(".minecraft/") {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "seed destination is outside the instance minecraft root",
            ));
        }
        if materialized_destinations.contains(&destination.to_ascii_lowercase()) {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "seed destination collides with a managed materialization",
            )
            .with_context("destination", destination.to_owned()));
        }
        if layer.expected_size == 0 {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "seed layer declares an empty payload",
            ));
        }
    }

    Ok(())
}

fn receipt_components_match(plan: &InstallPlan) -> bool {
    if plan.receipt.components.len() != plan.minecraft.components.len() {
        return false;
    }

    plan.receipt
        .components
        .iter()
        .zip(&plan.minecraft.components)
        .all(|(installed, resolved)| {
            let kind_matches = match resolved.kind {
                graphene_minecraft::ComponentKind::Minecraft => matches!(
                    installed.kind,
                    graphene_instance::InstalledComponentKind::Minecraft
                ),
                graphene_minecraft::ComponentKind::Loader => matches!(
                    installed.kind,
                    graphene_instance::InstalledComponentKind::Loader
                ),
                graphene_minecraft::ComponentKind::Auxiliary => matches!(
                    installed.kind,
                    graphene_instance::InstalledComponentKind::Auxiliary
                ),
                _ => false,
            };
            kind_matches
                && installed.uid == resolved.uid.as_str()
                && installed.version == resolved.version.as_str()
                && installed.provider == resolved.provenance.provider
                && installed.provenance == resolved.provenance.detail
        })
}

fn validate_preparation(plan: &InstallPlan, ids: &HashMap<ArtifactId, &Artifact>) -> Result<()> {
    const MAX_PROCESSORS: usize = 128;
    const MAX_OUTPUTS: usize = 512;
    const MAX_CLASSPATH: usize = 1024;
    const MAX_ARGUMENTS: usize = 4096;

    if plan.preparation.processors.len() > MAX_PROCESSORS
        || plan.preparation.generated_outputs.len() > MAX_OUTPUTS
        || plan.preparation.embedded_inputs.len() > MAX_OUTPUTS
        || plan.preparation.data.len() > 1024
    {
        return Err(install_error(
            ErrorCode::InstallPlanInvalid,
            "loader preparation exceeds resource limits",
        ));
    }

    let has_preparation = !plan.preparation.processors.is_empty()
        || !plan.preparation.embedded_inputs.is_empty()
        || !plan.preparation.generated_outputs.is_empty();
    if has_preparation
        && (plan.preparation.component.is_none() || plan.preparation.installer.is_none())
    {
        return Err(install_error(
            ErrorCode::InstallPlanInvalid,
            "loader preparation requires exact component and verified installer identity",
        ));
    }

    if let Some(component) = &plan.preparation.component {
        let matches = plan
            .minecraft
            .components
            .iter()
            .any(|candidate| candidate == component);
        if !matches || !matches!(component.kind, graphene_minecraft::ComponentKind::Loader) {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "loader preparation component does not match the composed Minecraft state",
            ));
        }
    }

    if let Some(installer) = &plan.preparation.installer
        && !ids.contains_key(&installer.artifact.id)
    {
        return Err(install_error(
            ErrorCode::InstallPlanInvalid,
            "loader installer artifact is missing from the plan",
        ));
    }

    if plan.preparation.input_artifacts.len() > 4096
        || plan
            .preparation
            .input_artifacts
            .iter()
            .any(|artifact| !ids.contains_key(&artifact.artifact.id))
    {
        return Err(install_error(
            ErrorCode::InstallPlanInvalid,
            "loader preparation input artifacts are incomplete or oversized",
        ));
    }

    let mut embedded_paths = HashSet::new();
    for input in &plan.preparation.embedded_inputs {
        if input.entry.is_empty()
            || input.entry.len() > 1024
            || input.entry.contains('\\')
            || input.entry.starts_with('/')
            || input
                .entry
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || input.maximum_size == 0
            || input.maximum_size > 64 * 1024 * 1024
            || !input.staging_path.as_str().starts_with("inputs/")
            || !embedded_paths.insert(input.staging_path.as_str())
        {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "loader embedded input declaration is invalid",
            ));
        }

        minecraft_path_to_platform(&input.staging_path)?;
    }

    let output_ids = plan
        .preparation
        .generated_outputs
        .iter()
        .map(|output| output.id.as_str())
        .collect::<HashSet<_>>();
    if output_ids.len() != plan.preparation.generated_outputs.len() {
        return Err(install_error(
            ErrorCode::InstallPlanInvalid,
            "loader generated output IDs are not unique",
        ));
    }

    for processor in &plan.preparation.processors {
        if processor.id.is_empty()
            || processor.id.len() > 128
            || processor.id.contains('\0')
            || processor.classpath.len() > MAX_CLASSPATH
            || processor.arguments.len() > MAX_ARGUMENTS
            || processor.timeout_seconds == 0
            || processor.timeout_seconds > 3600
        {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "loader processor declaration is invalid",
            ));
        }

        if !ids.contains_key(&processor.executable_jar.artifact.id)
            || processor
                .classpath
                .iter()
                .any(|artifact| !ids.contains_key(&artifact.artifact.id))
            || processor
                .declared_outputs
                .iter()
                .any(|output| !output_ids.contains(output.as_str()))
        {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "loader processor references undeclared input or output",
            ));
        }
    }

    let processor_ids = plan
        .preparation
        .processors
        .iter()
        .map(|processor| processor.id.as_str())
        .collect::<HashSet<_>>();
    for output in &plan.preparation.generated_outputs {
        if output.id.is_empty()
            || output.id.len() > 128
            || output.id.contains('\0')
            || output.input_identity.is_empty()
            || output.input_identity.len() > 16 * 1024
            || output.input_identity.contains('\0')
            || output
                .expected_size
                .is_some_and(|size| size > crate::generated::MAX_GENERATED_OUTPUT_BYTES)
            || !processor_ids.contains(output.producer.as_str())
        {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "generated output identity is invalid",
            ));
        }

        minecraft_path_to_platform(&output.staging_path)?;
        minecraft_path_to_platform(&output.managed_destination)?;

        let destination = output.managed_destination.as_str();
        match output.scope {
            graphene_minecraft::GeneratedOutputScope::SharedImmutable
                if !destination.starts_with("shared/") =>
            {
                return Err(install_error(
                    ErrorCode::InstallPlanInvalid,
                    "shared generated output is outside the shared root",
                ));
            }
            graphene_minecraft::GeneratedOutputScope::InstanceStaging
                if !(destination.starts_with(".minecraft/")
                    || destination.starts_with(".graphene/")) =>
            {
                return Err(install_error(
                    ErrorCode::InstallPlanInvalid,
                    "instance generated output is outside the instance root",
                ));
            }
            graphene_minecraft::GeneratedOutputScope::StagingOnly => {}
            _ => {}
        }
    }

    Ok(())
}

fn validate_plan_materialization_completeness(plan: &InstallPlan) -> Result<()> {
    let mut expected = HashSet::<(ArtifactId, String, MaterializationScope)>::new();
    let mut expect = |artifact: &ResolvedArtifact, scope: MaterializationScope| {
        expected.insert((
            artifact.artifact.id,
            artifact.relative_path.as_str().to_owned(),
            scope,
        ));
    };

    let generated_destinations = plan
        .preparation
        .generated_outputs
        .iter()
        .filter(|output| {
            matches!(
                output.scope,
                graphene_minecraft::GeneratedOutputScope::SharedImmutable
            )
        })
        .map(|output| output.managed_destination.as_str())
        .collect::<HashSet<_>>();

    expect(&plan.minecraft.client, MaterializationScope::Instance);
    for library in &plan.minecraft.libraries {
        if let Some(classpath) = &library.classpath_artifact
            && !generated_destinations.contains(classpath.relative_path.as_str())
        {
            expect(classpath, MaterializationScope::Shared);
        }

        if let Some(native) = &library.native_artifact {
            expect(native, MaterializationScope::Shared);
        }
    }

    expect(&plan.minecraft.assets.index, MaterializationScope::Shared);
    for object in &plan.minecraft.assets.objects {
        expect(&object.artifact, MaterializationScope::Shared);
    }

    if let Some(logging) = &plan.minecraft.logging {
        expect(&logging.artifact, MaterializationScope::Shared);
    }

    for metadata in &plan.metadata_artifacts {
        expect(metadata, MaterializationScope::Shared);
    }

    let actual = plan
        .shared_materializations
        .iter()
        .chain(&plan.instance_materializations)
        .map(|entry| {
            (
                entry.artifact_id,
                entry.destination.as_str().to_owned(),
                entry.scope,
            )
        })
        .collect::<HashSet<_>>();
    if actual.len() != plan.shared_materializations.len() + plan.instance_materializations.len() {
        return Err(install_error(
            ErrorCode::InstallPlanInvalid,
            "installation materializations contain exact duplicates",
        ));
    }

    // The resolved base model must be fully represented; composite plans may additionally carry
    // pack-managed instance payload, but only inside the managed instance root.
    for entry in &expected {
        if !actual.contains(entry) {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "installation materializations do not exactly match the resolved artifact model",
            ));
        }
    }
    for (artifact_id, destination, scope) in &actual {
        if expected.contains(&(*artifact_id, destination.clone(), *scope)) {
            continue;
        }
        if *scope != MaterializationScope::Instance || !destination.starts_with(".minecraft/") {
            return Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "extra materialization is outside the pack-managed instance root",
            )
            .with_context("destination", destination.clone()));
        }
    }

    let expected_natives = plan
        .minecraft
        .libraries
        .iter()
        .filter_map(|library| library.native_artifact.as_ref())
        .map(|native| {
            (
                native.artifact.id,
                plan.receipt.natives_directory.as_str().to_owned(),
            )
        })
        .collect::<HashSet<_>>();
    let actual_natives = plan
        .native_extractions
        .iter()
        .map(|entry| (entry.artifact_id, entry.destination.as_str().to_owned()))
        .collect::<HashSet<_>>();

    if actual_natives.len() != plan.native_extractions.len() || actual_natives != expected_natives {
        return Err(install_error(
            ErrorCode::InstallPlanInvalid,
            "native extraction plan does not exactly match selected native artifacts",
        ));
    }

    Ok(())
}
pub(super) fn validate_materialization_path(materialization: &Materialization) -> Result<()> {
    minecraft_path_to_platform(&materialization.destination)?;
    let path = materialization.destination.as_str();
    match materialization.scope {
        MaterializationScope::Shared if !path.starts_with("shared/") => Err(install_error(
            ErrorCode::InstallPlanInvalid,
            "shared materialization is outside the shared root",
        )),
        MaterializationScope::Instance
            if !(path.starts_with(".minecraft/") || path.starts_with(".graphene/")) =>
        {
            Err(install_error(
                ErrorCode::InstallPlanInvalid,
                "instance materialization is outside the instance-managed roots",
            ))
        }
        _ => Ok(()),
    }
}
