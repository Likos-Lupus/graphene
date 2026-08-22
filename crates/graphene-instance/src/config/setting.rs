use serde::{Deserialize, Serialize};

/// Tri-state update for configuration fields.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", content = "value", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SettingUpdate<T> {
    /// Leaves the existing configured setting unchanged.
    #[default]
    Unchanged,
    /// Explicitly sets the configuration to a specific value.
    Set(T),
    /// Removes the explicit override, resetting to inherit from the higher-precedence tier.
    Inherit,
}

impl<T: Clone> SettingUpdate<T> {
    /// Applies this update to an optional target value.
    pub fn apply_to(&self, target: &mut Option<T>) {
        match self {
            Self::Unchanged => {}
            Self::Set(value) => *target = Some(value.clone()),
            Self::Inherit => *target = None,
        }
    }
}
