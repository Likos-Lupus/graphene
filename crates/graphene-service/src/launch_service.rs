use crate::{JavaService, context::ServiceContext};
use graphene_core::Result;
use graphene_java::JavaRuntime;
use graphene_launch::{
    DEFAULT_GAME_EVENT_CAPACITY, LaunchPlan, LaunchRequest, RunningGame, execute,
    plan_from_committed,
};
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
        plan_from_committed(self.context.storage.path(), &request, java).await
    }

    pub fn execute(&self, plan: LaunchPlan) -> Result<RunningGame> {
        execute(plan, DEFAULT_GAME_EVENT_CAPACITY)
    }
}
