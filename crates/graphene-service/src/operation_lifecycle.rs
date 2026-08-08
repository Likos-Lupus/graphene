use graphene_core::{
    ErrorCode, ErrorKind, GrapheneError, OperationController, OperationResult, Result,
};

pub(crate) fn start(controller: &OperationController) -> Result<()> {
    match controller.start() {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = controller.cancelled();
            Err(error)
        }
    }
}

pub(crate) fn finish<T>(
    controller: &OperationController,
    result: Result<T>,
    cancelled_code: ErrorCode,
    cancelled_kind: ErrorKind,
    cancelled_message: &'static str,
) -> Result<T> {
    match result {
        Ok(value) => match controller.succeed()? {
            OperationResult::Succeeded => Ok(value),
            OperationResult::Cancelled => Err(GrapheneError::new(
                cancelled_code,
                cancelled_kind,
                cancelled_message,
            )),
            OperationResult::Failed { .. } => Err(GrapheneError::new(
                ErrorCode::InternalInvariantViolation,
                ErrorKind::Internal,
                "operation failed during success completion",
            )),
        },
        Err(error) if error.is_cancelled() || controller.is_cancelled() => {
            let _ = controller.cancelled();
            Err(GrapheneError::new(
                cancelled_code,
                cancelled_kind,
                cancelled_message,
            ))
        }
        Err(error) => {
            let _ = controller.fail(error.summary());
            Err(error)
        }
    }
}
