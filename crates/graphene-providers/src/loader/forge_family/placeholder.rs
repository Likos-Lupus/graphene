use crate::loader::common::loader_error;
use graphene_core::{ErrorCode, Result};
use graphene_minecraft::{
    MavenCoordinate, PreparationArgument, PreparationArgumentPart, PreparationPlaceholder,
};

const MAX_ARGUMENT_BYTES: usize = 16 * 1024;
const MAX_TEMPLATE_PARTS: usize = 128;

pub(super) fn parse_argument(value: &str) -> Result<PreparationArgument> {
    if value.len() > MAX_ARGUMENT_BYTES || value.contains('\0') {
        return Err(invalid("loader processor argument is invalid"));
    }

    if value.starts_with('[') && value.ends_with(']') && value.len() > 2 {
        let coordinate = MavenCoordinate::parse(&value[1..value.len() - 1]).map_err(|source| {
            invalid("loader processor Maven placeholder is invalid").with_source(source)
        })?;

        return Ok(PreparationArgument::Placeholder(
            PreparationPlaceholder::MavenPath(coordinate),
        ));
    }

    let mut parts = Vec::new();
    let mut cursor = 0usize;

    while let Some(relative_start) = value[cursor..].find('{') {
        let start = cursor + relative_start;
        if start > cursor {
            parts.push(PreparationArgumentPart::Literal(
                value[cursor..start].to_owned(),
            ));
        }

        let Some(relative_end) = value[start + 1..].find('}') else {
            return Err(invalid("loader processor placeholder is unterminated"));
        };

        let end = start + 1 + relative_end;
        let name = &value[start + 1..end];

        if name.is_empty() || name.len() > 128 || name.contains('{') || name.contains('}') {
            return Err(invalid("loader processor placeholder name is invalid"));
        }

        parts.push(PreparationArgumentPart::Placeholder(
            parse_named_placeholder(name)?,
        ));

        cursor = end + 1;
        if parts.len() > MAX_TEMPLATE_PARTS {
            return Err(invalid(
                "loader processor argument contains too many placeholder parts",
            ));
        }
    }

    if cursor == 0 {
        return Ok(PreparationArgument::Literal(value.to_owned()));
    }

    if cursor < value.len() {
        parts.push(PreparationArgumentPart::Literal(value[cursor..].to_owned()));
    }

    if parts.len() == 1
        && let PreparationArgumentPart::Placeholder(value) = parts.remove(0)
    {
        return Ok(PreparationArgument::Placeholder(value));
    }

    Ok(PreparationArgument::Template(parts))
}

fn parse_named_placeholder(name: &str) -> Result<PreparationPlaceholder> {
    Ok(match name {
        "ROOT" => PreparationPlaceholder::Root,
        "INSTALLER" => PreparationPlaceholder::Installer,
        "LIBRARY_DIR" => PreparationPlaceholder::LibraryDir,
        "MINECRAFT_JAR" => PreparationPlaceholder::MinecraftJar,
        "MINECRAFT_VERSION" => PreparationPlaceholder::MinecraftVersion,
        "SIDE" => PreparationPlaceholder::Side,
        other
            if other
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')) =>
        {
            PreparationPlaceholder::Data(other.to_owned())
        }
        _ => return Err(invalid("loader processor placeholder is not allowlisted")),
    })
}

fn invalid(message: &'static str) -> graphene_core::GrapheneError {
    loader_error(ErrorCode::LoaderProcessorPlaceholderInvalid, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_placeholders_without_shell_syntax() {
        assert!(matches!(
            parse_argument("{ROOT}").expect("root"),
            PreparationArgument::Placeholder(PreparationPlaceholder::Root)
        ));
        assert!(matches!(
            parse_argument("[org.example:tool:1]").expect("maven"),
            PreparationArgument::Placeholder(PreparationPlaceholder::MavenPath(_))
        ));
        assert!(matches!(
            parse_argument("--input={MINECRAFT_JAR}").expect("template"),
            PreparationArgument::Template(_)
        ));
        assert_eq!(
            parse_argument("{BAD/PATH}").expect_err("unknown").code,
            ErrorCode::LoaderProcessorPlaceholderInvalid
        );
    }
}
