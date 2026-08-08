use crate::{JavaVendor, error::java_error};
use graphene_core::{ErrorCode, Result};

/// Parses Java 8's `1.8` convention and modern integer-major forms.
pub fn parse_java_major(version: &str) -> Result<u32> {
    let version = version.trim().trim_matches('"');
    if version.is_empty() || version.len() > 128 {
        return Err(java_error(
            ErrorCode::JavaProbeFailed,
            "Java version string is invalid",
        ));
    }

    let major_text = if let Some(rest) = version.strip_prefix("1.") {
        rest.split(|ch: char| !ch.is_ascii_digit())
            .next()
            .unwrap_or_default()
    } else {
        version
            .split(|ch: char| !ch.is_ascii_digit())
            .next()
            .unwrap_or_default()
    };

    let major = major_text.parse::<u32>().map_err(|source| {
        java_error(
            ErrorCode::JavaProbeFailed,
            "Java version major is malformed",
        )
        .with_source(source)
    })?;

    if major == 0 || major > 10_000 {
        return Err(java_error(
            ErrorCode::JavaProbeFailed,
            "Java version major is out of range",
        ));
    }

    Ok(major)
}

#[must_use]
pub fn normalize_vendor(value: &str) -> JavaVendor {
    let trimmed = value.trim();
    let normalized = trimmed.to_ascii_lowercase();

    let contains_any = |needles: &[&str]| needles.iter().any(|needle| normalized.contains(needle));

    if contains_any(&["adoptium", "temurin", "eclipse"]) {
        JavaVendor::Adoptium
    } else if contains_any(&["oracle"]) {
        JavaVendor::Oracle
    } else if contains_any(&["microsoft"]) {
        JavaVendor::Microsoft
    } else if contains_any(&["azul", "zulu"]) {
        JavaVendor::Azul
    } else if contains_any(&["amazon", "corretto"]) {
        JavaVendor::Amazon
    } else if contains_any(&["graal"]) {
        JavaVendor::GraalVm
    } else {
        JavaVendor::Other(trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JavaVendor;

    #[test]
    fn parses_java_8_and_modern_majors() {
        assert_eq!(parse_java_major("1.8.0_402").expect("8"), 8);
        assert_eq!(parse_java_major("17.0.11").expect("17"), 17);
        assert_eq!(parse_java_major("21").expect("21"), 21);
        assert_eq!(parse_java_major("123.4").expect("future"), 123);
    }

    #[test]
    fn malformed_version_is_rejected() {
        assert_eq!(
            parse_java_major("not-java").expect_err("invalid").code,
            ErrorCode::JavaProbeFailed
        );
    }

    #[test]
    fn normalizes_vendor() {
        assert_eq!(normalize_vendor("Eclipse Adoptium"), JavaVendor::Adoptium);
        assert_eq!(normalize_vendor("Microsoft"), JavaVendor::Microsoft);
    }
}
