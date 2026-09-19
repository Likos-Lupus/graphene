//! Deterministic, fail-closed secret and path redaction for host-visible diagnostic text.

use super::text::truncate_head;
use graphene_core::SensitiveString;

/// Replacement emitted for any recognized secret value.
pub const SECRET_PLACEHOLDER: &str = "<redacted>";
/// Stable placeholder for the explicit Graphene data root.
pub const DATA_ROOT_PLACEHOLDER: &str = "<data-root>";
/// Stable placeholder for the user home prefix.
pub const HOME_PLACEHOLDER: &str = "<home>";

/// Case-insensitive key names whose values are treated as secret-bearing.
const SENSITIVE_KEYS: &[&str] = &[
    "access_token",
    "refresh_token",
    "auth_token",
    "id_token",
    "client_secret",
    "clientsecret",
    "authorization",
    "api_key",
    "apikey",
    "x-api-key",
    "password",
    "passwd",
    "secret",
    "token",
    "session",
    "device_code",
    "user_code",
];

/// Inputs that seed deterministic redaction.
///
/// Debug output never reveals the wrapped secret values.
#[derive(Debug, Clone, Default)]
pub struct RedactionContext {
    secrets: Vec<SensitiveString>,
    path_prefixes: Vec<(String, String)>,
}

impl RedactionContext {
    /// Creates an empty context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one known secret value. Empty values are ignored.
    #[must_use]
    pub fn with_secret(mut self, secret: SensitiveString) -> Self {
        if !secret.is_empty() {
            self.secrets.push(secret);
        }
        self
    }

    /// Adds known secret values.
    #[must_use]
    pub fn with_secrets(mut self, secrets: impl IntoIterator<Item = SensitiveString>) -> Self {
        for secret in secrets {
            self = self.with_secret(secret);
        }
        self
    }

    /// Adds a literal path prefix and its stable placeholder. Empty prefixes are ignored.
    #[must_use]
    pub fn with_path_prefix(
        mut self,
        prefix: impl Into<String>,
        placeholder: impl Into<String>,
    ) -> Self {
        let prefix = prefix.into();
        if !prefix.is_empty() {
            self.path_prefixes.push((prefix, placeholder.into()));
        }
        self
    }

    /// Adds the explicit Graphene data root prefix.
    #[must_use]
    pub fn with_data_root(self, data_root: impl Into<String>) -> Self {
        self.with_path_prefix(data_root, DATA_ROOT_PLACEHOLDER)
    }

    /// Adds the user home prefix.
    #[must_use]
    pub fn with_home(self, home: impl Into<String>) -> Self {
        self.with_path_prefix(home, HOME_PLACEHOLDER)
    }

    /// Returns whether no redaction inputs were configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.secrets.is_empty() && self.path_prefixes.is_empty()
    }
}

/// A bounded excerpt that has already been redacted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactedExcerpt {
    pub text: String,
    pub truncated: bool,
}

/// Precompiled deterministic redactor. Output never falls back to raw text.
pub struct Redactor {
    secrets: Vec<String>,
    prefixes: Vec<(String, String)>,
}

impl std::fmt::Debug for Redactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Redactor")
            .field("secret_count", &self.secrets.len())
            .field("prefix_count", &self.prefixes.len())
            .finish()
    }
}

impl Redactor {
    /// Compiles a redactor from a context. Longer secrets and prefixes win over shorter ones.
    #[must_use]
    pub fn new(context: &RedactionContext) -> Self {
        let mut secrets: Vec<String> = context
            .secrets
            .iter()
            .map(|secret| secret.expose_secret().to_owned())
            .filter(|secret| !secret.is_empty())
            .collect();
        secrets.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
        secrets.dedup();

        let mut prefixes = context.path_prefixes.clone();
        prefixes.sort_by(|left, right| {
            right
                .0
                .len()
                .cmp(&left.0.len())
                .then_with(|| left.0.cmp(&right.0))
        });
        prefixes.dedup();

        Self { secrets, prefixes }
    }

    /// Redacts an arbitrary string. The operation is deterministic and idempotent.
    #[must_use]
    pub fn redact(&self, input: &str) -> String {
        let mut value = input.to_owned();
        for secret in &self.secrets {
            if value.contains(secret.as_str()) {
                value = value.replace(secret.as_str(), SECRET_PLACEHOLDER);
            }
        }

        for (prefix, placeholder) in &self.prefixes {
            if value.contains(prefix.as_str()) {
                value = value.replace(prefix.as_str(), placeholder);
            }
        }

        let value = redact_key_values(&value);
        let value = redact_auth_schemes(&value);

        redact_url_userinfo(&value)
    }

    /// Redacts then bounds an excerpt to `max_bytes` on a char boundary.
    #[must_use]
    pub fn excerpt(&self, input: &str, max_bytes: usize) -> RedactedExcerpt {
        let redacted = self.redact(input);
        let (text, truncated) = truncate_head(&redacted, max_bytes);
        RedactedExcerpt {
            text: text.to_owned(),
            truncated,
        }
    }
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

fn is_value_delimiter(byte: u8) -> bool {
    byte.is_ascii_whitespace()
        || matches!(
            byte,
            b'"' | b'\'' | b'&' | b',' | b';' | b'}' | b')' | b']' | b'#' | b'|' | b'`' | 0
        )
}

fn is_authority_end(byte: u8) -> bool {
    byte.is_ascii_whitespace()
        || matches!(byte, b'/' | b'?' | b'#' | b'<' | b'>' | b'"' | b'\'' | 0)
}

fn skip_whitespace(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    pos
}

fn starts_with_ignore_case(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.len() >= needle.len() && haystack[..needle.len()].eq_ignore_ascii_case(needle)
}

/// Finds the value span after a sensitive key, or `None` if the key is not a key-value pair.
fn match_secret_value(input: &str, after_key: usize) -> Option<(usize, usize)> {
    let bytes = input.as_bytes();
    let mut pos = skip_whitespace(bytes, after_key);

    if pos < bytes.len() && (bytes[pos] == b'"' || bytes[pos] == b'\'') {
        pos += 1;
    }

    pos = skip_whitespace(bytes, pos);
    if pos >= bytes.len() || (bytes[pos] != b'=' && bytes[pos] != b':') {
        return None;
    }

    pos += 1;
    pos = skip_whitespace(bytes, pos);
    if pos < bytes.len() && (bytes[pos] == b'"' || bytes[pos] == b'\'') {
        let quote = bytes[pos];
        pos += 1;
        let start = pos;
        while pos < bytes.len() && bytes[pos] != quote {
            pos += 1;
        }

        return (pos > start).then_some((start, pos));
    }

    let start = pos;
    let auth_scheme = starts_with_ignore_case(&bytes[start..], b"bearer ")
        || starts_with_ignore_case(&bytes[start..], b"basic ")
        || starts_with_ignore_case(&bytes[start..], b"digest ");
    if auth_scheme {
        while pos < bytes.len() && !matches!(bytes[pos], b'\r' | b'\n' | 0) {
            pos += 1;
        }
    } else {
        while pos < bytes.len() && !is_value_delimiter(bytes[pos]) {
            pos += 1;
        }
    }

    (pos > start).then_some((start, pos))
}

fn redact_key_values(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut index = 0;
    while index < bytes.len() {
        if is_ident_start(bytes[index]) {
            let start = index;
            let mut end = index;

            while end < bytes.len() && is_ident_byte(bytes[end]) {
                end += 1;
            }

            let run = &input[start..end];
            let is_sensitive = SENSITIVE_KEYS.contains(&run.to_ascii_lowercase().as_str());

            if is_sensitive && let Some((value_start, value_end)) = match_secret_value(input, end) {
                out.push_str(run);
                out.push_str(&input[end..value_start]);
                out.push_str(SECRET_PLACEHOLDER);
                index = value_end;
                continue;
            }

            out.push_str(run);
            index = end;
        } else {
            let ch = input[index..].chars().next().unwrap_or('\u{fffd}');
            out.push(ch);
            index += ch.len_utf8();
        }
    }
    out
}

fn redact_auth_schemes(input: &str) -> String {
    let value = redact_scheme_token(input, "bearer");
    let value = redact_scheme_token(&value, "basic");
    redact_scheme_token(&value, "digest")
}

fn redact_scheme_token(input: &str, scheme: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let bytes = input.as_bytes();

    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;

    while let Some(relative) = lower[cursor..].find(scheme) {
        let start = cursor + relative;
        out.push_str(&input[cursor..start]);
        let after = start + scheme.len();
        out.push_str(&input[start..after]);
        let boundary_before = start == 0 || !is_ident_byte(bytes[start - 1]);
        if boundary_before {
            let mut end = after;
            while end < bytes.len() && bytes[end].is_ascii_whitespace() {
                out.push(bytes[end] as char);
                end += 1;
            }

            let value_start = end;
            while end < bytes.len() && !is_value_delimiter(bytes[end]) {
                end += 1;
            }

            if end > value_start {
                out.push_str(SECRET_PLACEHOLDER);
            }

            cursor = end;
        } else {
            cursor = after;
        }
    }

    out.push_str(&input[cursor..]);
    out
}

fn redact_url_userinfo(input: &str) -> String {
    let bytes = input.as_bytes();

    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;

    while let Some(relative) = input[cursor..].find("://") {
        let scheme_end = cursor + relative;
        out.push_str(&input[cursor..scheme_end + 3]);
        let authority_start = scheme_end + 3;

        let mut authority_end = authority_start;
        while authority_end < bytes.len() && !is_authority_end(bytes[authority_end]) {
            authority_end += 1;
        }

        let authority = &input[authority_start..authority_end];
        match authority.rfind('@') {
            Some(at) => {
                let userinfo = &authority[..at];
                if userinfo.is_empty() || userinfo.contains(SECRET_PLACEHOLDER) {
                    out.push_str(userinfo);
                } else {
                    out.push_str(SECRET_PLACEHOLDER);
                }

                out.push('@');
                out.push_str(&authority[at + 1..]);
            }
            None => out.push_str(authority),
        }

        cursor = authority_end;
    }

    out.push_str(&input[cursor..]);
    out
}
