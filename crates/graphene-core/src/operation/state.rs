use serde::{Deserialize, Serialize};

/// Lifecycle state shared by every long-running Graphene operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OperationState {
    Created,
    Queued,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
}

impl OperationState {
    /// Returns whether this state is terminal.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }

    /// Returns whether the documented lifecycle permits a direct transition.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Created, Self::Queued)
                | (Self::Created, Self::Running)
                | (Self::Created, Self::Cancelled)
                | (Self::Queued, Self::Running)
                | (Self::Queued, Self::Cancelled)
                | (Self::Running, Self::Succeeded)
                | (Self::Running, Self::Failed)
                | (Self::Running, Self::Cancelling)
                | (Self::Cancelling, Self::Cancelled)
                | (Self::Cancelling, Self::Failed)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states_cannot_transition() {
        for state in [
            OperationState::Succeeded,
            OperationState::Failed,
            OperationState::Cancelled,
        ] {
            for next in [
                OperationState::Created,
                OperationState::Queued,
                OperationState::Running,
                OperationState::Cancelling,
                OperationState::Succeeded,
                OperationState::Failed,
                OperationState::Cancelled,
            ] {
                assert!(!state.can_transition_to(next));
            }
        }
    }
}
