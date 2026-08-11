//! Stable, infrastructure-independent foundation types for Graphene.
//!
//! This crate owns identifiers, artifact declarations, integrity values, structured errors and
//! diagnostics, and the single operation/progress/event/cancellation model used by later phases.

mod artifact;
mod diagnostic;
mod error;
mod hash;
mod id;
pub mod operation;
mod secret;

pub use artifact::{Artifact, ArtifactIntegrity, ArtifactKind, ArtifactSource, CachePolicy};
pub use diagnostic::{Diagnostic, DiagnosticCode, DiagnosticParameters, DiagnosticSeverity};
pub use error::{ErrorCode, ErrorContext, ErrorKind, ErrorSummary, GrapheneError};
pub use hash::{HashParseError, Sha1Digest, Sha256Digest};
pub use id::{AccountId, ArtifactId, IdParseError, InstanceId, ManagedRuntimeId, OperationId};
pub use operation::{
    CancellationToken, OperationController, OperationEvent, OperationEventKind,
    OperationEventStream, OperationHandle, OperationRegistry, OperationResult, OperationSnapshot,
    OperationState, Progress,
};
pub use secret::SensitiveString;

/// Graphene's common result type.
pub type Result<T> = std::result::Result<T, GrapheneError>;
