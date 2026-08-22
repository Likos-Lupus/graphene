use crate::{InstallReceipt, InstanceDescriptor, InstanceStatus};
use serde::{Deserialize, Serialize};

/// Stable in-memory record for a committed instance loaded through the repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommittedInstance {
    pub descriptor: InstanceDescriptor,
    pub receipt: InstallReceipt,
    pub status: InstanceStatus,
}

impl CommittedInstance {
    /// Creates a new committed instance record.
    #[must_use]
    pub fn new(
        descriptor: InstanceDescriptor,
        receipt: InstallReceipt,
        status: InstanceStatus,
    ) -> Self {
        Self {
            descriptor,
            receipt,
            status,
        }
    }
}
