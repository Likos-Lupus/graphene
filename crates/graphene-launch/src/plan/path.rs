use crate::error::launch_error;
use graphene_core::{ErrorCode, Result};
use graphene_instance::ManagedRelativePath;
use graphene_minecraft::{MinecraftArch, MinecraftOs};
use graphene_platform::{Architecture, OperatingSystem, normalize_process_path};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn resolve_persisted_path(
    data_root: &Path,
    instance_root: &Path,
    path: &ManagedRelativePath,
) -> Result<PathBuf> {
    let relative = Path::new(path.as_str());
    if path.as_str().starts_with("shared/") {
        reject_symlink_components(data_root, relative)?;
        Ok(data_root.join(relative))
    } else if path.as_str().starts_with(".minecraft/") || path.as_str().starts_with(".graphene/") {
        reject_symlink_components(instance_root, relative)?;
        Ok(instance_root.join(relative))
    } else {
        Err(launch_error(
            ErrorCode::LaunchInstanceInvalid,
            "persisted path is outside recognized managed roots",
        ))
    }
}

pub(super) fn reject_symlink_components(root: &Path, relative: &Path) -> Result<()> {
    let root_metadata = fs::symlink_metadata(root).map_err(|source| {
        launch_error(
            ErrorCode::LaunchInstanceInvalid,
            "managed launch root is unavailable",
        )
        .with_source(source)
    })?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(launch_error(
            ErrorCode::LaunchInstanceInvalid,
            "managed launch root is not a real directory",
        ));
    }

    let mut current = root.to_path_buf();
    for component in relative.components() {
        use std::path::Component;
        let Component::Normal(component) = component else {
            return Err(launch_error(
                ErrorCode::LaunchInstanceInvalid,
                "persisted launch path escaped its managed root",
            ));
        };

        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|source| {
            launch_error(
                ErrorCode::LaunchInstanceInvalid,
                "persisted launch path is unavailable",
            )
            .with_source(source)
        })?;
        if metadata.file_type().is_symlink() {
            return Err(launch_error(
                ErrorCode::LaunchInstanceInvalid,
                "persisted launch path contains a symbolic link",
            ));
        }
    }

    Ok(())
}

pub(super) fn path_utf8(path: &Path) -> Result<String> {
    normalize_process_path(path)
        .into_os_string()
        .into_string()
        .map_err(|_| {
            launch_error(
                ErrorCode::LaunchPlanInvalid,
                "launch path is not valid UTF-8",
            )
        })
}

pub(super) fn validate_ordinary_file(
    path: &Path,
    code: ErrorCode,
    message: &'static str,
) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| launch_error(code, message).with_source(source))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(launch_error(code, message));
    }

    Ok(())
}

pub(super) fn validate_directory(path: &Path, label: &'static str) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|source| {
        launch_error(
            ErrorCode::LaunchInstanceInvalid,
            "required launch directory is unavailable",
        )
        .with_context("path_role", label)
        .with_source(source)
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(launch_error(
            ErrorCode::LaunchInstanceInvalid,
            "required launch path is not a directory",
        )
        .with_context("path_role", label));
    }

    Ok(())
}

pub(super) fn minecraft_os(os: OperatingSystem) -> MinecraftOs {
    match os {
        OperatingSystem::Windows => MinecraftOs::Windows,
        OperatingSystem::Linux => MinecraftOs::Linux,
        OperatingSystem::MacOS => MinecraftOs::Osx,
        OperatingSystem::Other => MinecraftOs::Other("other".into()),
    }
}

pub(super) fn minecraft_arch(arch: Architecture) -> MinecraftArch {
    match arch {
        Architecture::X86 => MinecraftArch::X86,
        Architecture::X86_64 => MinecraftArch::X86_64,
        Architecture::AArch64 => MinecraftArch::AArch64,
        Architecture::Other => MinecraftArch::Other("other".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::build::instance_working_directory;
    use std::path::{Path, PathBuf};

    #[test]
    fn working_directory_is_the_instance_minecraft_directory() {
        assert_eq!(
            instance_working_directory(Path::new("instances/fixture")),
            PathBuf::from("instances/fixture/.minecraft")
        );
    }
}
