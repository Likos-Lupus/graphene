use crate::error::HostError;
use graphene::{
    Account, AccountId, AuthInteraction, ContentProviderId, InstanceId, InstanceInventoryEntry,
};
use graphene_reference_host_support::RunSummary;
use serde::{Deserialize, Serialize};

/// Engine identity and resolved data root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineInfo {
    pub data_root: String,
    pub os: String,
    pub architecture: String,
}

/// Handle returned when a long-running operation is started; events are correlated by this id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationHandleView {
    pub operation_id: String,
}

/// App-local instance inventory projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceSummary {
    pub instance_id: String,
    pub display_name: Option<String>,
    pub status: String,
    pub minecraft_version: Option<String>,
}

impl From<&InstanceInventoryEntry> for InstanceSummary {
    fn from(entry: &InstanceInventoryEntry) -> Self {
        Self {
            instance_id: entry.instance_id.to_string(),
            display_name: entry.display_name.clone(),
            status: format!("{:?}", entry.status),
            minecraft_version: entry.minecraft_version.clone(),
        }
    }
}

/// App-local account projection; never carries tokens or credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSummary {
    pub account_id: String,
    pub kind: String,
    pub state: String,
    pub display_name: String,
}

impl From<&Account> for AccountSummary {
    fn from(account: &Account) -> Self {
        Self {
            account_id: account.id.to_string(),
            kind: format!("{:?}", account.kind),
            state: format!("{:?}", account.state),
            display_name: account.profile.display_name.clone(),
        }
    }
}

/// The minimum intended authentication interaction crossing to the webview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthInteractionView {
    pub operation_id: String,
    pub kind: String,
    pub verification_uri: String,
    pub user_code: String,
    pub expires_in_seconds: u64,
    pub poll_interval_seconds: u64,
}

impl AuthInteractionView {
    #[must_use]
    pub fn from_interaction(operation_id: String, interaction: &AuthInteraction) -> Self {
        match interaction {
            AuthInteraction::DeviceAuthorization {
                verification_uri,
                user_code,
                expires_in_seconds,
                poll_interval_seconds,
                ..
            } => Self {
                operation_id,
                kind: "device-authorization".to_owned(),
                verification_uri: verification_uri.clone(),
                user_code: user_code.expose_secret().to_owned(),
                expires_in_seconds: *expires_in_seconds,
                poll_interval_seconds: *poll_interval_seconds,
            },
            
            _ => Self {
                operation_id,
                kind: "unknown".to_owned(),
                verification_uri: String::new(),
                user_code: String::new(),
                expires_in_seconds: 0,
                poll_interval_seconds: 0,
            },
        }
    }
}

/// Opaque host run identity; never exposes Tokio child handles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunHandleView {
    pub run_id: String,
    pub pid: u32,
}

/// Bounded run summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSummaryView {
    pub run_id: String,
    pub pid: u32,
    pub dropped_output_count: u64,
}

impl From<RunSummary> for RunSummaryView {
    fn from(summary: RunSummary) -> Self {
        Self {
            run_id: summary.run_id,
            pid: summary.pid,
            dropped_output_count: summary.dropped_output_count,
        }
    }
}

pub fn parse_instance_id(value: &str) -> Result<InstanceId, HostError> {
    value
        .parse::<InstanceId>()
        .map_err(|_| HostError::invalid("instance_id", value))
}

pub fn parse_account_id(value: &str) -> Result<AccountId, HostError> {
    value
        .parse::<AccountId>()
        .map_err(|_| HostError::invalid("account_id", value))
}

pub fn parse_provider_id(value: &str) -> Result<ContentProviderId, HostError> {
    ContentProviderId::new(value).map_err(|_| HostError::invalid("provider", value))
}
