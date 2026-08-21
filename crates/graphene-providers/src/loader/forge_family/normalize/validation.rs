use super::*;

pub(super) fn validate_key(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(loader_error(
            ErrorCode::LoaderInstallerInvalid,
            "loader installer data key is invalid",
        ));
    }

    Ok(())
}

pub(super) fn validate_text(value: &str, maximum: usize, message: &'static str) -> Result<()> {
    if value.is_empty() || value.len() > maximum || value.contains('\0') {
        return Err(loader_error(ErrorCode::LoaderProfileInvalid, message));
    }

    Ok(())
}
