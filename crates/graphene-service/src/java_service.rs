use crate::context::ServiceContext;
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, InstanceId, Result};
use graphene_instance::InstallReceipt;
use graphene_java::{JavaRequirement, JavaRuntime, select_java};
use std::{path::PathBuf, sync::Arc};

#[derive(Clone)]
pub struct JavaService {
    context: Arc<ServiceContext>,
}

impl std::fmt::Debug for JavaService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JavaService").finish_non_exhaustive()
    }
}

impl JavaService {
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        Self { context }
    }

    pub async fn select_for_instance(
        &self,
        instance_id: InstanceId,
        explicit: Option<PathBuf>,
    ) -> Result<JavaRuntime> {
        let receipt_path = self
            .context
            .storage
            .path()
            .join("instances")
            .join(instance_id.to_string())
            .join(".graphene/install.json");
        let bytes = tokio::fs::read(&receipt_path).await.map_err(|source| {
            GrapheneError::new(
                ErrorCode::LaunchInstanceInvalid,
                ErrorKind::Launch,
                "committed install receipt is unavailable for Java selection",
            )
            .with_source(source)
        })?;
        let receipt = InstallReceipt::from_json(&bytes)?;

        if receipt.instance_id != instance_id {
            return Err(GrapheneError::new(
                ErrorCode::LaunchInstanceInvalid,
                ErrorKind::Launch,
                "install receipt identity does not match Java selection request",
            ));
        }

        let requirement = JavaRequirement {
            major_version: receipt.java_requirement.major_version,
            component_hint: receipt.java_requirement.component_hint.clone(),
        };
        select_java(&requirement, explicit).await
    }
}
