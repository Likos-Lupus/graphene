use crate::InstanceDescriptor;

/// Stable in-memory identity for a committed instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedInstance {
    pub descriptor: InstanceDescriptor,
}
