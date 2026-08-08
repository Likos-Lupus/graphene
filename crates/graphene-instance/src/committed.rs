use crate::InstanceDescriptor;

/// Stable in-memory identity for a committed Phase 1 instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedInstance {
    pub descriptor: InstanceDescriptor,
}
