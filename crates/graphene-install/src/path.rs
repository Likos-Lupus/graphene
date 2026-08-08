use crate::error::install_error;
use graphene_core::{ErrorCode, Result};

pub(crate) fn minecraft_path_to_platform(
    path: &graphene_minecraft::ManagedPath,
) -> Result<graphene_platform::ManagedRelativePath> {
    graphene_platform::ManagedRelativePath::new(path.as_str()).map_err(|source| {
        install_error(
            ErrorCode::InstallPlanInvalid,
            "planned path is not a safe managed-relative path",
        )
        .with_source(source)
    })
}

pub(crate) fn receipt_path_to_platform(
    path: &graphene_instance::ManagedRelativePath,
) -> Result<graphene_platform::ManagedRelativePath> {
    graphene_platform::ManagedRelativePath::new(path.as_str()).map_err(|source| {
        install_error(
            ErrorCode::InstallPlanInvalid,
            "persisted receipt path is not a safe managed-relative path",
        )
        .with_source(source)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipt_and_minecraft_paths_convert_without_collapsing_domain_types() {
        let minecraft = graphene_minecraft::ManagedPath::new(".graphene/natives/fixture")
            .expect("minecraft path");
        let receipt = graphene_instance::ManagedRelativePath::new(".graphene/natives/fixture")
            .expect("receipt path");

        assert_eq!(
            minecraft_path_to_platform(&minecraft)
                .expect("minecraft conversion")
                .as_str(),
            ".graphene/natives/fixture"
        );
        assert_eq!(
            receipt_path_to_platform(&receipt)
                .expect("receipt conversion")
                .as_str(),
            ".graphene/natives/fixture"
        );
    }
}
