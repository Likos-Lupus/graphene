use crate::{Account, AccountState};
use graphene_core::{
    Diagnostic, DiagnosticCode, DiagnosticParameters, DiagnosticSeverity, ErrorCode, GrapheneError,
};

/// Produces stable account-state evidence without embedding host-facing prose.
#[must_use]
pub fn account_diagnostic(account: &Account) -> Diagnostic {
    let (code, severity) = match account.state {
        AccountState::Ready => ("AUTH_ACCOUNT_READY", DiagnosticSeverity::Info),
        AccountState::RequiresReauthentication => (
            "AUTH_REAUTHENTICATION_REQUIRED",
            DiagnosticSeverity::Warning,
        ),
        AccountState::SecretStoreUnavailable => {
            ("AUTH_SECRET_STORE_UNAVAILABLE", DiagnosticSeverity::Error)
        }
    };
    Diagnostic {
        code: DiagnosticCode::new(code),
        severity,
        parameters: DiagnosticParameters::new(),
    }
}

/// Normalizes actionable authentication failures into stable diagnostic codes.
#[must_use]
pub fn authentication_error_diagnostic(error: &GrapheneError) -> Diagnostic {
    let (code, severity) = match error.code {
        ErrorCode::AuthRefreshRejected | ErrorCode::AuthInteractionRequired => (
            "AUTH_REAUTHENTICATION_REQUIRED",
            DiagnosticSeverity::Warning,
        ),
        ErrorCode::AuthSecretStoreUnavailable
        | ErrorCode::AuthSecretReadFailed
        | ErrorCode::AuthSecretWriteFailed
        | ErrorCode::AuthSecretDeleteFailed => {
            ("AUTH_SECRET_STORE_UNAVAILABLE", DiagnosticSeverity::Error)
        }
        ErrorCode::AuthEntitlementMissing => {
            ("AUTH_ENTITLEMENT_MISSING", DiagnosticSeverity::Error)
        }
        ErrorCode::AuthProfileMissing => ("AUTH_PROFILE_MISSING", DiagnosticSeverity::Error),
        ErrorCode::AuthProviderUnavailable => (
            "AUTH_PROVIDER_TEMPORARILY_UNAVAILABLE",
            DiagnosticSeverity::Warning,
        ),
        _ => ("AUTH_FAILURE", DiagnosticSeverity::Error),
    };
    Diagnostic {
        code: DiagnosticCode::new(code),
        severity,
        parameters: DiagnosticParameters::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccountKind, AccountProfile};
    use graphene_core::{AccountId, ErrorKind};
    use uuid::Uuid;

    #[test]
    fn reauthentication_and_provider_outage_are_distinct() {
        let account = Account {
            id: AccountId::new(),
            kind: AccountKind::Microsoft,
            profile: AccountProfile {
                display_name: "FixturePlayer".into(),
                minecraft_uuid: Uuid::new_v4(),
            },
            state: AccountState::RequiresReauthentication,
        };
        assert_eq!(
            account_diagnostic(&account).code.as_str(),
            "AUTH_REAUTHENTICATION_REQUIRED"
        );

        let outage = GrapheneError::new(
            ErrorCode::AuthProviderUnavailable,
            ErrorKind::Authentication,
            "temporary",
        );
        assert_eq!(
            authentication_error_diagnostic(&outage).code.as_str(),
            "AUTH_PROVIDER_TEMPORARILY_UNAVAILABLE"
        );
    }
}
