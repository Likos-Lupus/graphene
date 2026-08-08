use crate::{ErrorCode, ErrorKind, GrapheneError, Result};
use serde::{Deserialize, Serialize};

/// UI-neutral progress value for a long-running operation stage.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Progress {
    #[default]
    Indeterminate,
    Items {
        completed: u64,
        total: Option<u64>,
    },
    Bytes {
        completed: u64,
        total: Option<u64>,
    },
}

impl Progress {
    /// Validates the `completed <= total` invariant when a total is known.
    pub fn validate(&self) -> Result<()> {
        let pair = match self {
            Self::Indeterminate => return Ok(()),
            Self::Items { completed, total } | Self::Bytes { completed, total } => {
                (*completed, *total)
            }
        };
        match pair.1 {
            Some(total) if pair.0 > total => Err(GrapheneError::new(
                ErrorCode::InternalInvariantViolation,
                ErrorKind::Internal,
                "progress completed value exceeds its total",
            )
            .with_context("completed", pair.0.to_string())
            .with_context("total", total.to_string())),
            _ => Ok(()),
        }
    }

    /// Returns whether `next` is monotonic relative to `self` within the same stage.
    #[must_use]
    pub fn is_monotonic_to(&self, next: &Self) -> bool {
        match (self, next) {
            (Self::Indeterminate, _) => true,
            (Self::Items { completed: a, .. }, Self::Items { completed: b, .. })
            | (Self::Bytes { completed: a, .. }, Self::Bytes { completed: b, .. }) => b >= a,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_total_cannot_be_exceeded() {
        let progress = Progress::Bytes {
            completed: 11,
            total: Some(10),
        };
        assert_eq!(
            progress.validate().expect_err("must reject").code,
            ErrorCode::InternalInvariantViolation
        );
    }

    #[test]
    fn progress_is_monotonic_per_variant() {
        let a = Progress::Items {
            completed: 3,
            total: Some(10),
        };
        let b = Progress::Items {
            completed: 4,
            total: Some(10),
        };
        assert!(a.is_monotonic_to(&b));
        assert!(!b.is_monotonic_to(&a));
    }
}
