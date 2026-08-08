use super::path::{
    minecraft_arch, minecraft_os, path_utf8, reject_symlink_components, resolve_persisted_path,
    validate_directory, validate_ordinary_file,
};
use super::placeholder::{PlaceholderValues, expand_logging_argument, resolve_arguments};
use super::{EnvironmentDelta, LaunchArgument, LaunchPlan};
use crate::{error::launch_error, request::LaunchRequest};
use graphene_core::{ErrorCode, Result};
use graphene_instance::{InstallReceipt, InstanceDescriptor};
use graphene_java::JavaRuntime;
use graphene_minecraft::RuleContext;
use graphene_platform::{Platform, classpath_separator};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

/// Reconstructs launch state exclusively from committed local metadata and local paths.
pub async fn plan_from_committed(
    data_root: &Path,
    request: &LaunchRequest,
    java: JavaRuntime,
) -> Result<LaunchPlan> {
    request.validate()?;
    let instance_root = data_root
        .join("instances")
        .join(request.instance_id.to_string());
    validate_directory(data_root, "data root")?;
    validate_directory(&data_root.join("instances"), "instances root")?;
    validate_directory(&instance_root, "instance root")?;
    reject_symlink_components(&instance_root, Path::new("instance.json"))?;
    reject_symlink_components(&instance_root, Path::new(".graphene/install.json"))?;
    let descriptor_bytes = tokio::fs::read(instance_root.join("instance.json"))
        .await
        .map_err(|source| {
            launch_error(
                ErrorCode::LaunchInstanceInvalid,
                "committed instance descriptor is unavailable",
            )
            .with_source(source)
        })?;
    let descriptor: InstanceDescriptor =
        serde_json::from_slice(&descriptor_bytes).map_err(|source| {
            launch_error(
                ErrorCode::LaunchInstanceInvalid,
                "committed instance descriptor is invalid",
            )
            .with_source(source)
        })?;
    descriptor.validate().map_err(|source| {
        launch_error(
            ErrorCode::LaunchInstanceInvalid,
            "committed instance descriptor failed validation",
        )
        .with_source(source)
    })?;

    if descriptor.instance_id != request.instance_id {
        return Err(launch_error(
            ErrorCode::LaunchInstanceInvalid,
            "committed instance identity does not match the request",
        ));
    }

    let receipt_bytes = tokio::fs::read(instance_root.join(".graphene/install.json"))
        .await
        .map_err(|source| {
            launch_error(
                ErrorCode::LaunchInstanceInvalid,
                "committed install receipt is unavailable",
            )
            .with_source(source)
        })?;
    let receipt = InstallReceipt::from_json(&receipt_bytes).map_err(|source| {
        launch_error(
            ErrorCode::LaunchInstanceInvalid,
            "committed install receipt is invalid",
        )
        .with_source(source)
    })?;

    if receipt.instance_id != request.instance_id {
        return Err(launch_error(
            ErrorCode::LaunchInstanceInvalid,
            "install receipt identity does not match the request",
        ));
    }

    let data_root = data_root.to_path_buf();
    let request = request.clone();

    tokio::task::spawn_blocking(move || {
        build_plan(&data_root, &instance_root, &request, &receipt, java)
    })
    .await
    .map_err(|source| {
        launch_error(
            ErrorCode::LaunchPlanInvalid,
            "launch filesystem validation worker failed",
        )
        .with_source(source)
    })?
}

pub(super) fn instance_working_directory(instance_root: &Path) -> PathBuf {
    instance_root.join(".minecraft")
}

fn build_plan(
    data_root: &Path,
    instance_root: &Path,
    request: &LaunchRequest,
    receipt: &InstallReceipt,
    java: JavaRuntime,
) -> Result<LaunchPlan> {
    let working_directory = instance_working_directory(instance_root);

    reject_symlink_components(instance_root, Path::new(".minecraft"))?;
    reject_symlink_components(instance_root, Path::new(receipt.natives_directory.as_str()))?;
    reject_symlink_components(data_root, Path::new(receipt.assets_root.as_str()))?;
    reject_symlink_components(data_root, Path::new("shared/libraries"))?;

    let natives_directory = instance_root.join(receipt.natives_directory.as_str());
    let assets_root = data_root.join(receipt.assets_root.as_str());
    let libraries_root = data_root.join("shared/libraries");

    validate_directory(&working_directory, "working directory")?;
    validate_directory(&assets_root, "assets root")?;

    if !receipt
        .libraries
        .iter()
        .all(|library| library.native_archive.is_none())
    {
        validate_directory(&natives_directory, "native directory")?;
    } else if !natives_directory.exists() {
        // A stable path is still supplied to argument templates even when no native artifacts were
        // selected. Creating it is installation's responsibility; an empty directory is valid.
        return Err(launch_error(
            ErrorCode::LaunchInstanceInvalid,
            "committed native directory is unavailable",
        ));
    }

    let mut classpath = Vec::<PathBuf>::new();
    let mut seen = HashSet::<PathBuf>::new();

    for library in &receipt.libraries {
        if let Some(classpath_artifact) = &library.classpath {
            let path = resolve_persisted_path(data_root, instance_root, &classpath_artifact.path)?;
            if seen.insert(path.clone()) {
                classpath.push(path);
            }
        }
    }

    let client = resolve_persisted_path(data_root, instance_root, &receipt.client.path)?;
    if seen.insert(client.clone()) {
        classpath.push(client);
    }

    for path in &classpath {
        validate_ordinary_file(
            path,
            ErrorCode::LaunchInstanceInvalid,
            "committed classpath entry is unavailable",
        )?;
    }

    let platform = Platform::current();
    let mut features = BTreeMap::new();
    features.insert(
        "has_custom_resolution".to_owned(),
        request.resolution.is_some(),
    );
    features.insert("is_demo_user".to_owned(), false);
    features.insert("has_quick_plays_support".to_owned(), false);
    features.insert("is_quick_play_singleplayer".to_owned(), false);
    features.insert("is_quick_play_multiplayer".to_owned(), false);
    features.insert("is_quick_play_realms".to_owned(), false);
    let rules = RuleContext {
        os: minecraft_os(platform.os),
        arch: minecraft_arch(platform.architecture),
        os_version: None,
        features,
    };

    let separator = classpath_separator();
    let values = PlaceholderValues {
        username: request.session.username.clone(),
        uuid: request.session.uuid.clone(),
        access_token: request.session.access_token.clone(),
        user_type: request.session.user_type.clone(),
        client_id: request.session.client_id.clone(),
        xuid: request.session.xuid.clone(),
        version_name: receipt.resolved_version.to_string(),
        version_type: receipt.version_type.clone(),
        game_directory: path_utf8(&working_directory)?,
        assets_root: path_utf8(&assets_root)?,
        asset_index_name: receipt.asset_index_id.clone(),
        natives_directory: path_utf8(&natives_directory)?,
        library_directory: path_utf8(&libraries_root)?,
        resolution: request.resolution,
    };

    let mut jvm_args = resolve_arguments(&receipt.jvm_arguments, &rules, &values)?;

    if !jvm_args
        .iter()
        .any(|argument| matches!(argument, LaunchArgument::Classpath))
    {
        jvm_args.push(LaunchArgument::Plain("-cp".to_owned()));
        jvm_args.push(LaunchArgument::Classpath);
    }

    if let (Some(logging), Some(template)) =
        (&receipt.logging_configuration, &receipt.logging_argument)
    {
        let path = resolve_persisted_path(data_root, instance_root, &logging.path)?;
        validate_ordinary_file(
            &path,
            ErrorCode::LaunchInstanceInvalid,
            "logging configuration is unavailable",
        )?;
        let path = path_utf8(&path)?;
        jvm_args.push(expand_logging_argument(template, &path)?);
    }

    jvm_args.extend(
        request
            .extra_jvm_args
            .iter()
            .cloned()
            .map(LaunchArgument::Plain),
    );

    let mut game_args = resolve_arguments(&receipt.game_arguments, &rules, &values)?;
    game_args.extend(
        request
            .extra_game_args
            .iter()
            .cloned()
            .map(LaunchArgument::Plain),
    );

    let plan = LaunchPlan {
        instance_id: request.instance_id,
        java,
        working_directory,
        environment: EnvironmentDelta::default(),
        jvm_args,
        classpath,
        classpath_separator: separator,
        main_class: receipt.main_class.clone(),
        game_args,
        natives_directory,
    };
    plan.validate()?;

    Ok(plan)
}
