use super::limits::{MAX_ENTRY_NAME_BYTES, MAX_PATH_DEPTH};
use crate::error::PackError;
use serde::{Deserialize, Serialize};

/// A validated pack-relative destination path.
///
/// Construction is fallible and total: every raw archive name or manifest destination passes
/// through [`PackPath::normalize`] exactly once during normalization; validated instances can be
/// assumed safe for staging under the instance payload root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PackPath(String);

/// Windows reserved device names that must never appear as a path component prefix.
const WINDOWS_RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

impl PackPath {
    /// Validates and normalizes one raw pack-relative path.
    pub fn normalize(raw: &str) -> Result<Self, PackError> {
        if raw.is_empty() {
            return Err(PackError::path("pack path is empty"));
        }

        if raw.len() > MAX_ENTRY_NAME_BYTES {
            return Err(PackError::path("pack path exceeds the length bound"));
        }

        if raw.contains('\\') {
            return Err(PackError::path("pack path contains a backslash separator"));
        }

        if raw.starts_with('/') {
            return Err(PackError::path("pack path is absolute"));
        }

        if raw.contains(':') {
            return Err(PackError::path(
                "pack path contains a drive/stream separator",
            ));
        }

        if raw.ends_with('/') {
            return Err(PackError::path("pack path designates a directory"));
        }

        let mut depth = 0_usize;
        for component in raw.split('/') {
            validate_component(component)?;
            depth += 1;
            if depth > MAX_PATH_DEPTH {
                return Err(PackError::path("pack path exceeds the depth bound"));
            }
        }

        Ok(Self(raw.to_owned()))
    }

    /// The normalized forward-slash path string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Path components in order.
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    /// Final component (file name).
    #[must_use]
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or_default()
    }

    /// Parent directory path, if any.
    #[must_use]
    pub fn parent(&self) -> Option<&str> {
        let cut = self.0.rfind('/')?;
        Some(&self.0[..cut])
    }

    /// Case-folded key used to detect portable filesystem collisions.
    #[must_use]
    pub fn collision_key(&self) -> String {
        self.0.to_lowercase()
    }

    /// Returns whether this path is inside the given directory prefix.
    #[must_use]
    pub fn starts_under(&self, directory: &str) -> bool {
        debug_assert!(!directory.is_empty());
        self.0.starts_with(directory)
            && (self.0.len() == directory.len()
                || self.0.as_bytes().get(directory.len()) == Some(&b'/'))
    }
}

fn validate_component(component: &str) -> Result<(), PackError> {
    if component.is_empty() || component == "." || component == ".." {
        return Err(PackError::path(
            "pack path contains an empty or traversal component",
        ));
    }

    let trimmed = component.trim_end_matches(['.', ' ']);

    if trimmed.is_empty() {
        return Err(PackError::path(
            "pack path component consists only of dots/spaces",
        ));
    }

    if trimmed != component {
        // Trailing dot/space aliases resolve differently on Windows.
        return Err(PackError::path(
            "pack path component ends with a dot/space alias",
        ));
    }

    for ch in component.chars() {
        if ch.is_control() || ch == '\u{7f}' {
            return Err(PackError::path("pack path contains a control character"));
        }
    }

    let lower = trimmed.to_ascii_lowercase();
    let stem = lower.split('.').next().unwrap_or_default();
    if WINDOWS_RESERVED.contains(&stem) {
        return Err(PackError::path("pack path uses a reserved device name"));
    }

    Ok(())
}

/// Detects whether all entries share one unambiguous top-level wrapper directory and returns the
/// wrapper name. Multiple distinct candidate wrappers make stripping ambiguous.
pub fn detect_wrapper_root(entry_names: &[&str]) -> Option<String> {
    let mut wrapper: Option<&str> = None;
    for name in entry_names {
        let Some(slash) = name.find('/') else {
            return None; // a root-level loose file means there is no wrapper
        };

        let first = &name[..slash];
        match wrapper {
            None => wrapper = Some(first),
            Some(existing) if existing == first => {}
            Some(_) => return None,
        }
    }
    wrapper.map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphene_core::ErrorCode;

    fn err_of(raw: &str) -> graphene_core::ErrorCode {
        PackPath::normalize(raw).expect_err("must reject").code()
    }

    #[test]
    fn accepts_ordinary_relative_paths() {
        let path = PackPath::normalize("config/optifine.cfg").expect("valid");
        assert_eq!(path.as_str(), "config/optifine.cfg");
        assert_eq!(path.file_name(), "optifine.cfg");
        assert_eq!(path.parent(), Some("config"));
        assert!(path.starts_under("config"));
        assert!(!path.starts_under("configs"));
    }

    #[test]
    fn rejects_traversal_and_absolute_forms() {
        assert_eq!(err_of("../escape.txt"), ErrorCode::PackPathInvalid);
        assert_eq!(err_of("a/../../b"), ErrorCode::PackPathInvalid);
        assert_eq!(err_of("/absolute"), ErrorCode::PackPathInvalid);
        assert_eq!(err_of("C:/x"), ErrorCode::PackPathInvalid);
        assert_eq!(err_of("a\\..\\b"), ErrorCode::PackPathInvalid);
        assert_eq!(err_of(""), ErrorCode::PackPathInvalid);
        assert_eq!(err_of("dir/"), ErrorCode::PackPathInvalid);
        assert_eq!(err_of("a//b"), ErrorCode::PackPathInvalid);
        assert_eq!(err_of("./a"), ErrorCode::PackPathInvalid);
        assert_eq!(err_of("name "), ErrorCode::PackPathInvalid);
        assert_eq!(err_of("name."), ErrorCode::PackPathInvalid);
        assert_eq!(err_of("CON"), ErrorCode::PackPathInvalid);
        assert_eq!(err_of("mod s/con.txt"), ErrorCode::PackPathInvalid);
    }

    #[test]
    fn collision_keys_are_case_folded() {
        assert_eq!(
            PackPath::normalize("Mods/A.Jar")
                .expect("valid")
                .collision_key(),
            PackPath::normalize("mods/a.jar")
                .expect("valid")
                .collision_key()
        );
    }

    #[test]
    fn wrapper_detection_is_all_or_nothing() {
        let names = vec!["pack/mods/a.jar", "pack/config/x.cfg"];
        assert_eq!(detect_wrapper_root(&names), Some("pack".to_owned()));
        let ambiguous = vec!["one/a", "two/b"];
        assert_eq!(detect_wrapper_root(&ambiguous), None);
        let loose = vec!["loose.txt", "dir/b"];
        assert_eq!(detect_wrapper_root(&loose), None);
    }
}
