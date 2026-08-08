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
            if planned.artifact.sources.is_empty() || !planned.artifact.integrity.is_verifiable() {
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
        validate_plan_materialization_completeness(self)?;

        Ok(())
    }
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

    expect(&plan.minecraft.client, MaterializationScope::Instance);
    for library in &plan.minecraft.libraries {
        if let Some(classpath) = &library.classpath_artifact {
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
    if actual.len() != plan.shared_materializations.len() + plan.instance_materializations.len()
        || actual != expected
    {
        return Err(install_error(
            ErrorCode::InstallPlanInvalid,
            "installation materializations do not exactly match the resolved artifact model",
        ));
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
