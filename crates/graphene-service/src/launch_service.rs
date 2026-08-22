use crate::{JavaService, context::ServiceContext, instance_service::InstanceRepository};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use graphene_instance::{InstanceLockfile, InstanceStateFingerprint};
use graphene_java::JavaRuntime;
use graphene_launch::{
    DEFAULT_GAME_EVENT_CAPACITY, LaunchPlan, LaunchRequest, RunningGame, execute_with_lease,
    plan_with_committed,
};
use graphene_storage::{MAX_LOCKFILE_BYTES, read_document_bounded};
use std::sync::Arc;

#[derive(Clone)]
pub struct LaunchService {
    context: Arc<ServiceContext>,
}

impl std::fmt::Debug for LaunchService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LaunchService").finish_non_exhaustive()
    }
}

impl LaunchService {
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        Self { context }
    }

    /// Selects Java locally and reconstructs a launch plan from committed metadata without network.
    pub async fn plan(&self, request: LaunchRequest) -> Result<LaunchPlan> {
        let java = JavaService::new(Arc::clone(&self.context))
            .select_for_instance(request.instance_id, request.java_override.clone())
            .await?;
        self.plan_with_java(request, java).await
    }

    /// Plans with an already selected runtime, useful for hosts that expose Java selection
    /// separately. This path remains completely offline.
    pub async fn plan_with_java(
        &self,
        request: LaunchRequest,
        java: JavaRuntime,
    ) -> Result<LaunchPlan> {
        let repo = InstanceRepository::new(self.context.storage.path());
        let _lease = repo.acquire_shared_lease(request.instance_id)?;
        let descriptor = repo.load_descriptor(request.instance_id)?;
        let receipt = repo.load_receipt(request.instance_id)?;

        let lockfile_path = repo.paths().lockfile_path(request.instance_id);
        let lockfile = if lockfile_path.exists() {
            let bytes = read_document_bounded(&lockfile_path, MAX_LOCKFILE_BYTES)?;
            serde_json::from_slice::<InstanceLockfile>(&bytes).ok()
        } else {
            None
        };

        let effective_config = repo.effective_config(request.instance_id)?;
        let fingerprint = InstanceStateFingerprint::compute(
            request.instance_id,
            &receipt,
            lockfile.as_ref(),
            Some(&effective_config),
        );

        let mut plan = plan_with_committed(
            self.context.storage.path(),
            &request,
            &descriptor,
            &receipt,
            java,
        )
        .await?;
        plan.state_fingerprint = Some(fingerprint);
        Ok(plan)
    }

    /// Spawns the game process while holding a shared advisory lease for its entire lifetime.
    ///
    /// Validates that the plan's state fingerprint matches current committed state on disk,
    /// rejecting stale plans with `ErrorCode::InstanceRepairPlanStale`.
    pub fn execute(&self, plan: LaunchPlan) -> Result<RunningGame> {
        let repo = InstanceRepository::new(self.context.storage.path());
        let lease = repo.acquire_shared_lease(plan.instance_id)?;

        let receipt = repo.load_receipt(plan.instance_id)?;
        let lockfile_path = repo.paths().lockfile_path(plan.instance_id);
        let lockfile = if lockfile_path.exists() {
            let bytes = read_document_bounded(&lockfile_path, MAX_LOCKFILE_BYTES)?;
            serde_json::from_slice::<InstanceLockfile>(&bytes).ok()
        } else {
            None
        };

        let effective_config = repo.effective_config(plan.instance_id)?;
        let current_fingerprint = InstanceStateFingerprint::compute(
            plan.instance_id,
            &receipt,
            lockfile.as_ref(),
            Some(&effective_config),
        );

        if let Some(planned_fp) = plan.state_fingerprint
            && planned_fp != current_fingerprint
        {
            return Err(GrapheneError::new(
                ErrorCode::InstanceRepairPlanStale,
                ErrorKind::Launch,
                "launch plan is stale: instance state has changed since planning",
            )
            .with_context("instance_id", plan.instance_id.to_string())
            .with_context("planned_fingerprint", planned_fp.to_string())
            .with_context("current_fingerprint", current_fingerprint.to_string()));
        }

        execute_with_lease(plan, DEFAULT_GAME_EVENT_CAPACITY, Some(lease.into_opaque()))
    }
}
