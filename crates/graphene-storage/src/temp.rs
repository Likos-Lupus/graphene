use graphene_core::{ArtifactId, OperationId};
use std::path::{Path, PathBuf};

pub(crate) fn download_temp_path(
    temporary_root: &Path,
    artifact_id: ArtifactId,
    operation_id: OperationId,
) -> PathBuf {
    temporary_root.join(format!("{artifact_id}-{operation_id}.part"))
}
