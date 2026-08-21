use crate::{
    ArtifactService, adapters::ServiceMetadataAcquirer, context::ServiceContext,
    install_service::current_rule_context, loader_service::LoaderService,
};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, OperationController, Result};
use graphene_install::{ComponentInstallRequest, InstallPlan};
use graphene_minecraft::{ComponentDescriptor, ComponentGraph, LoaderSupport, compose_minecraft};
use graphene_providers::MojangProvider;
use std::{fs, sync::Arc};

pub(crate) async fn plan_component_install(
    context: Arc<ServiceContext>,
    request: ComponentInstallRequest,
    controller: &OperationController,
) -> Result<InstallPlan> {
    controller.set_stage("validate-request")?;
    request.instance.validate().map_err(|source| {
        GrapheneError::new(
            ErrorCode::InstallRequestInvalid,
            ErrorKind::Install,
            "component install request contains invalid instance data",
        )
        .with_source(source)
    })?;
    reject_existing_target(&context, request.instance.id)?;

    controller.set_stage("resolve-minecraft")?;
    let provider = MojangProvider::new(context.network.clone(), context.provider_config.clone())?;
    let metadata = ServiceMetadataAcquirer::new(Arc::clone(&context));
    let rules = current_rule_context(context.platform.os, context.platform.architecture);
    let bundle = provider
        .resolve(&request.minecraft_version, &rules, &metadata, controller)
        .await?;

    controller.set_stage("resolve-loader")?;
    let kind = request.loader.kind;
    let mut loader = LoaderService::resolve_for_install(
        Arc::clone(&context),
        kind,
        &request.minecraft_version,
        request.loader.version.clone(),
        controller,
    )
    .await?;

    if let Some(installer) = loader.preparation.installer.as_ref() {
        controller.set_stage("acquire-loader-installer")?;
        let verified = ArtifactService::new(Arc::clone(&context))
            .acquire(installer.artifact.clone(), Some(&controller.handle()))
            .await_result()
            .await?;
        controller.set_stage("normalize-loader-installer")?;
        let loader_provider = context.loader_registry.provider(kind)?;
        loader = loader_provider
            .normalize_verified_installer(loader, verified.path, controller)
            .await?;
    }

    if !matches!(loader.support, LoaderSupport::Supported) {
        let reason = match &loader.support {
            LoaderSupport::MetadataOnly { reason } | LoaderSupport::Unsupported { reason } => {
                reason.clone()
            }
            LoaderSupport::Supported => String::new(),
            _ => "loader support classification is unknown".to_owned(),
        };

        return Err(GrapheneError::new(
            ErrorCode::LoaderVersionUnsupported,
            ErrorKind::Minecraft,
            "resolved loader release is not executable by this implementation",
        )
        .with_context("loader", kind.to_string())
        .with_context("loader_version", loader.version.to_string())
        .with_context("minecraft_version", request.minecraft_version.to_string())
        .with_context("reason", reason));
    }

    controller.set_stage("compose-components")?;
    let graph = ComponentGraph::new(vec![
        ComponentDescriptor::minecraft(request.minecraft_version.as_str())?,
        loader.component.clone(),
    ])?;

    let mut patches = Vec::new();
    for component in graph.ordered() {
        if component.uid == loader.component.uid {
            patches.push(loader.patch.clone());
        }
    }

    let minecraft = compose_minecraft(bundle.minecraft, &patches, &rules)?;

    controller.set_stage("build-plan")?;
    InstallPlan::build_component(
        request,
        minecraft,
        bundle.metadata_artifacts,
        loader.preparation,
    )
}

fn reject_existing_target(context: &ServiceContext, id: graphene_core::InstanceId) -> Result<()> {
    let target = context
        .storage
        .path()
        .join("instances")
        .join(id.to_string());

    match fs::symlink_metadata(&target) {
        Ok(_) => Err(GrapheneError::new(
            ErrorCode::InstallTargetExists,
            ErrorKind::Install,
            "instance target already exists",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(GrapheneError::new(
            ErrorCode::InstallStageFailed,
            ErrorKind::Install,
            "failed to inspect instance target",
        )
        .with_source(source)),
    }
}
