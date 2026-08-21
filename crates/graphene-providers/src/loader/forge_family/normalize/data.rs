use super::*;

pub(super) fn normalize_data(
    values: &BTreeMap<String, DataFileDto>,
    entries: &BTreeSet<&str>,
) -> Result<BTreeMap<String, PreparationDataValue>> {
    if values.len() > 1024 {
        return Err(loader_error(
            ErrorCode::LoaderInstallerInvalid,
            "loader installer data map exceeds its bound",
        ));
    }

    values
        .iter()
        .map(|(name, value)| {
            validate_key(name)?;
            Ok((
                name.clone(),
                PreparationDataValue::SideSpecific {
                    client: Box::new(parse_data_value(&value.client, entries)?),
                    server: value
                        .server
                        .as_deref()
                        .map(|value| parse_data_value(value, entries).map(Box::new))
                        .transpose()?,
                },
            ))
        })
        .collect()
}

fn parse_data_value(value: &str, entries: &BTreeSet<&str>) -> Result<PreparationDataValue> {
    validate_text(value, 16 * 1024, "loader installer data value is invalid")?;
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return Ok(PreparationDataValue::Literal(
            value[1..value.len() - 1].to_owned(),
        ));
    }

    if value.len() >= 2 && value.starts_with('[') && value.ends_with(']') {
        return MavenCoordinate::parse(&value[1..value.len() - 1])
            .map(PreparationDataValue::MavenCoordinate)
            .map_err(|source| {
                loader_error(
                    ErrorCode::LoaderInstallerInvalid,
                    "loader installer data contains an invalid Maven coordinate",
                )
                .with_source(source)
            });
    }

    if entries.contains(value) {
        return Ok(PreparationDataValue::EmbeddedInstallerEntry(
            value.to_owned(),
        ));
    }

    Ok(PreparationDataValue::Literal(value.to_owned()))
}

pub(super) fn output_digest(
    value: &str,
    data: &BTreeMap<String, PreparationDataValue>,
) -> Result<String> {
    let raw = if value.starts_with('{') && value.ends_with('}') {
        let name = &value[1..value.len() - 1];
        data_client_raw(data.get(name).ok_or_else(|| {
            loader_error(
                ErrorCode::LoaderProcessorOutputMismatch,
                "loader processor output digest references missing installer data",
            )
        })?)?
    } else {
        value.to_owned()
    };

    let value = raw.trim_matches('\'');
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(loader_error(
            ErrorCode::LoaderProcessorOutputMismatch,
            "loader processor output digest is not a SHA-1 value",
        ));
    }

    Ok(value.to_ascii_lowercase())
}

pub(super) fn data_client_raw(value: &PreparationDataValue) -> Result<String> {
    let value = match value {
        PreparationDataValue::SideSpecific { client, .. } => client.as_ref(),
        other => other,
    };

    Ok(match value {
        PreparationDataValue::Literal(value) => format!("'{value}'"),
        PreparationDataValue::MavenCoordinate(value) => format!("[{}]", value.to_string_key()),
        PreparationDataValue::EmbeddedInstallerEntry(value) => value.clone(),
        PreparationDataValue::ManagedPath(value) => value.as_str().to_owned(),
        PreparationDataValue::SideSpecific { .. } => {
            return Err(loader_error(
                ErrorCode::LoaderInstallerInvalid,
                "loader installer data contains nested side-specific data",
            ));
        }
        _ => {
            return Err(loader_error(
                ErrorCode::LoaderInstallerInvalid,
                "loader installer data variant is unsupported",
            ));
        }
    })
}
