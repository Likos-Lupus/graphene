pub mod discovery;
pub mod execution;
pub mod inventory;
pub mod planning;
pub mod recognition;
#[cfg(test)]
mod tests;
pub mod transaction;
pub mod update;

pub use execution::ContentMutationResult;
pub use transaction::{ContentFaultPoint, ContentMutationJournal};
pub use update::AvailableContentUpdate;

use crate::{context::ServiceContext, operation_lifecycle};
use graphene_content::{
    ContentFileMatch, ContentMutationPlan, ContentMutationRequest, ContentProject,
    ContentProjectRef, ContentProviderId, ContentSearchPage, ContentSearchQuery, ContentVersion,
    ContentVersionFilter, ContentVersionRef, LocalContentInventory, ReleaseChannelPolicy,
};
use graphene_core::{ErrorCode, ErrorKind, InstanceId, OperationHandle, Result};
use std::{future::Future, pin::Pin, sync::Arc};

macro_rules! define_content_operation {
    ($name:ident, $output:ty) => {
        pub struct $name {
            operation: OperationHandle,
            future: Pin<Box<dyn Future<Output = Result<$output>> + Send>>,
        }

        impl $name {
            #[must_use]
            pub fn operation(&self) -> OperationHandle {
                self.operation.clone()
            }

            pub async fn await_result(self) -> Result<$output> {
                self.future.await
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("operation_id", &self.operation.id())
                    .finish_non_exhaustive()
            }
        }
    };
}

define_content_operation!(ContentScanOperation, LocalContentInventory);
define_content_operation!(ContentSearchOperation, ContentSearchPage);
define_content_operation!(ContentProjectOperation, ContentProject);
define_content_operation!(ContentVersionsOperation, Vec<ContentVersion>);
define_content_operation!(ContentVersionOperation, ContentVersion);
define_content_operation!(ContentRecognitionOperation, Vec<ContentFileMatch>);
define_content_operation!(ContentPlanOperation, ContentMutationPlan);
define_content_operation!(ContentExecuteOperation, ContentMutationResult);
define_content_operation!(ContentUpdatesOperation, Vec<AvailableContentUpdate>);

/// Primary service facade for offline mod inventory and remote content catalog operations.
#[derive(Clone)]
pub struct ContentService {
    context: Arc<ServiceContext>,
}

impl std::fmt::Debug for ContentService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentService").finish_non_exhaustive()
    }
}

impl ContentService {
    #[must_use]
    pub(crate) fn new(context: Arc<ServiceContext>) -> Self {
        Self { context }
    }

    /// Scans an instance's local `.minecraft/mods` directory strictly offline.
    pub fn scan(&self, instance_id: InstanceId, compute_hashes: bool) -> ContentScanOperation {
        let controller = self.context.operations.create("content-scan");
        let operation = controller.handle();
        let ctx = Arc::clone(&self.context);
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result =
                inventory::scan_instance_inventory(&ctx, instance_id, compute_hashes, &controller)
                    .await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::OperationCancelled,
                ErrorKind::Cancelled,
                "content scan operation was cancelled",
            )
        });
        ContentScanOperation { operation, future }
    }

    /// Searches a remote content provider.
    pub fn search(
        &self,
        provider_id: &ContentProviderId,
        query: &ContentSearchQuery,
    ) -> ContentSearchOperation {
        let controller = self.context.operations.create("content-search");
        let operation = controller.handle();
        let ctx = Arc::clone(&self.context);
        let pid = provider_id.clone();
        let q = query.clone();
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = discovery::search_content(&ctx, &pid, &q, &controller).await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::OperationCancelled,
                ErrorKind::Cancelled,
                "content search operation was cancelled",
            )
        });
        ContentSearchOperation { operation, future }
    }

    /// Fetches project details from a provider.
    pub fn project(&self, project_ref: &ContentProjectRef) -> ContentProjectOperation {
        let controller = self.context.operations.create("content-project");
        let operation = controller.handle();
        let ctx = Arc::clone(&self.context);
        let pref = project_ref.clone();
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = discovery::get_content_project(&ctx, &pref, &controller).await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::OperationCancelled,
                ErrorKind::Cancelled,
                "content project lookup was cancelled",
            )
        });
        ContentProjectOperation { operation, future }
    }

    /// Lists project versions matching filter criteria.
    pub fn versions(
        &self,
        project_ref: &ContentProjectRef,
        filters: &ContentVersionFilter,
    ) -> ContentVersionsOperation {
        let controller = self.context.operations.create("content-versions");
        let operation = controller.handle();
        let ctx = Arc::clone(&self.context);
        let pref = project_ref.clone();
        let filt = filters.clone();
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = discovery::list_content_versions(&ctx, &pref, &filt, &controller).await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::OperationCancelled,
                ErrorKind::Cancelled,
                "content versions lookup was cancelled",
            )
        });
        ContentVersionsOperation { operation, future }
    }

    /// Fetches exact version details from a provider.
    pub fn version(&self, version_ref: &ContentVersionRef) -> ContentVersionOperation {
        let controller = self.context.operations.create("content-version");
        let operation = controller.handle();
        let ctx = Arc::clone(&self.context);
        let vref = version_ref.clone();
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = discovery::get_content_version(&ctx, &vref, &controller).await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::OperationCancelled,
                ErrorKind::Cancelled,
                "content version lookup was cancelled",
            )
        });
        ContentVersionOperation { operation, future }
    }

    /// Explicitly matches local mod hashes/fingerprints with remote providers.
    pub fn recognize(&self, instance_id: InstanceId) -> ContentRecognitionOperation {
        let controller = self.context.operations.create("content-recognize");
        let operation = controller.handle();
        let ctx = Arc::clone(&self.context);
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result =
                recognition::recognize_instance_files(&ctx, instance_id, &controller).await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::OperationCancelled,
                ErrorKind::Cancelled,
                "content recognition operation was cancelled",
            )
        });
        ContentRecognitionOperation { operation, future }
    }

    /// Derives a deterministic, non-mutating `ContentMutationPlan`.
    pub fn plan(&self, request: &ContentMutationRequest) -> ContentPlanOperation {
        let controller = self.context.operations.create("content-plan");
        let operation = controller.handle();
        let ctx = Arc::clone(&self.context);
        let req = request.clone();
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = planning::plan_content_mutation(&ctx, &req, &controller).await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::OperationCancelled,
                ErrorKind::Cancelled,
                "content planning operation was cancelled",
            )
        });
        ContentPlanOperation { operation, future }
    }

    /// Executes a content mutation plan transactionally under the exclusive lease.
    pub fn execute(&self, plan: ContentMutationPlan) -> ContentExecuteOperation {
        self.execute_with_fault(plan, None)
    }

    /// Executes a content mutation plan with an optional injected fault point for testing.
    pub fn execute_with_fault(
        &self,
        plan: ContentMutationPlan,
        fault: Option<ContentFaultPoint>,
    ) -> ContentExecuteOperation {
        let controller = self.context.operations.create("content-execute");
        let operation = controller.handle();
        let ctx = Arc::clone(&self.context);
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = execution::execute_content_mutation(&ctx, &plan, fault, &controller).await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::OperationCancelled,
                ErrorKind::Cancelled,
                "content execution operation was cancelled",
            )
        });
        ContentExecuteOperation { operation, future }
    }

    /// Finds available updates for all managed mods in an instance.
    pub fn find_updates(
        &self,
        instance_id: InstanceId,
        policy: ReleaseChannelPolicy,
    ) -> ContentUpdatesOperation {
        let controller = self.context.operations.create("content-find-updates");
        let operation = controller.handle();
        let ctx = Arc::clone(&self.context);
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result =
                update::find_available_updates(&ctx, instance_id, policy, &controller).await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::OperationCancelled,
                ErrorKind::Cancelled,
                "content find updates operation was cancelled",
            )
        });
        ContentUpdatesOperation { operation, future }
    }

    /// Plans a batch update for all available mod updates.
    pub fn plan_updates(
        &self,
        instance_id: InstanceId,
        policy: ReleaseChannelPolicy,
    ) -> ContentPlanOperation {
        let controller = self.context.operations.create("content-plan-updates");
        let operation = controller.handle();
        let ctx = Arc::clone(&self.context);
        let future = Box::pin(async move {
            operation_lifecycle::start(&controller)?;
            let result = update::plan_updates(&ctx, instance_id, policy, &controller).await;
            operation_lifecycle::finish(
                &controller,
                result,
                ErrorCode::OperationCancelled,
                ErrorKind::Cancelled,
                "content plan updates operation was cancelled",
            )
        });
        ContentPlanOperation { operation, future }
    }
}
