//! Provider-neutral Graphene account and authentication domain.
//!
//! Protocol adapters, persistence mechanics, and launch conversion stay outside this crate.

mod account;
mod diagnostic;
mod error;
mod offline;
mod provider;
mod refresh;
mod repository;
mod secret;

pub use account::{Account, AccountKind, AccountProfile, AccountState, OfflineAccountSpec};
pub use diagnostic::{account_diagnostic, authentication_error_diagnostic};
pub use offline::{create_offline_account, offline_auth_session, validate_offline_name};
pub use provider::{
    AuthChallenge, AuthFuture, AuthInteraction, AuthProvider, AuthProviderCapabilities, AuthSession,
};
pub use refresh::{RefreshDisposition, classify_refresh_error};
pub use repository::{AccountRepository, InMemoryAccountRepository};
pub use secret::{
    InMemorySecretStore, RefreshCredential, SecretRecordIdentity, SecretRecordKind, SecretStore,
    UnavailableSecretStore,
};
