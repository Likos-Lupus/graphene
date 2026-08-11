use graphene_core::{ErrorCode, GrapheneError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshDisposition {
    PreserveReadyState,
    RequireReauthentication,
}

#[must_use]
pub fn classify_refresh_error(error: &GrapheneError) -> RefreshDisposition {
    match error.code {
        ErrorCode::AuthRefreshRejected | ErrorCode::AuthInteractionRequired => {
            RefreshDisposition::RequireReauthentication
        }
        _ => RefreshDisposition::PreserveReadyState,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphene_core::{ErrorKind, GrapheneError};

    #[test]
    fn temporary_provider_failure_does_not_invalidate_account() {
        let error = GrapheneError::new(
            ErrorCode::AuthProviderUnavailable,
            ErrorKind::Authentication,
            "temporary",
        );
        assert_eq!(
            classify_refresh_error(&error),
            RefreshDisposition::PreserveReadyState
        );
    }
}
