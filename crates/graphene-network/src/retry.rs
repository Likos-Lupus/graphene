use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use std::time::Duration;

/// Bounded exponential retry policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Total attempts per source, including the first request.
    pub max_attempts: usize,
    /// Delay used before the first retry.
    pub base_delay: Duration,
    /// Upper bound for exponential backoff.
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(2),
        }
    }
}

impl RetryPolicy {
    /// Validates retry bounds.
    pub fn validate(&self) -> Result<()> {
        if !(1..=10).contains(&self.max_attempts) {
            return Err(config_error("retry max_attempts must be between 1 and 10"));
        }

        if self.base_delay.is_zero() || self.max_delay.is_zero() {
            return Err(config_error("retry delays must be non-zero"));
        }

        if self.base_delay > self.max_delay {
            return Err(config_error("retry base_delay must not exceed max_delay"));
        }

        if self.max_delay > Duration::from_secs(60) {
            return Err(config_error("retry max_delay must not exceed 60 seconds"));
        }

        Ok(())
    }

    /// Returns the deterministic bounded exponential delay before `attempt` (where attempt 2 is the
    /// first retry).
    #[must_use]
    pub fn delay_before_attempt(&self, attempt: usize) -> Duration {
        let exponent = attempt.saturating_sub(2).min(31) as u32;
        self.base_delay
            .saturating_mul(2_u32.saturating_pow(exponent))
            .min(self.max_delay)
    }
}

pub(crate) fn retryable_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 425 | 429 | 500 | 502 | 503 | 504)
}

fn config_error(message: &'static str) -> GrapheneError {
    GrapheneError::new(ErrorCode::ConfigInvalid, ErrorKind::Configuration, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_delay_is_bounded() {
        let policy = RetryPolicy {
            max_attempts: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(250),
        };

        assert_eq!(policy.delay_before_attempt(2), Duration::from_millis(100));
        assert_eq!(policy.delay_before_attempt(3), Duration::from_millis(200));
        assert_eq!(policy.delay_before_attempt(4), Duration::from_millis(250));
    }

    #[test]
    fn status_retry_classification_is_explicit() {
        assert!(retryable_status(reqwest::StatusCode::TOO_MANY_REQUESTS));
        assert!(retryable_status(reqwest::StatusCode::SERVICE_UNAVAILABLE));
        assert!(!retryable_status(reqwest::StatusCode::NOT_FOUND));
        assert!(!retryable_status(reqwest::StatusCode::BAD_REQUEST));
    }
}
