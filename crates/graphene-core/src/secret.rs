use std::fmt;

/// Ephemeral secret-bearing string whose ordinary formatting is always redacted.
#[derive(Clone, PartialEq, Eq)]
pub struct SensitiveString(String);

impl SensitiveString {
    /// Creates a new secret value. Empty values are allowed at this layer and validated by the
    /// owning use case.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Exposes the secret only at an explicit process-boundary call site.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    /// Returns whether the wrapped value is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SensitiveString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SensitiveString(<redacted>)")
    }
}

impl fmt::Display for SensitiveString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_never_reveals_secret() {
        let secret = SensitiveString::new("phase1-secret-value");
        assert!(!format!("{secret:?}").contains("phase1-secret-value"));
        assert!(!format!("{secret}").contains("phase1-secret-value"));
        assert_eq!(secret.expose_secret(), "phase1-secret-value");
    }
}
