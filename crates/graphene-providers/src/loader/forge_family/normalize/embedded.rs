use super::*;

pub(super) fn collect_embedded_inputs(
    data: &BTreeMap<String, PreparationDataValue>,
    entries: &BTreeSet<&str>,
    processors: &[ProcessorStep],
) -> Result<Vec<graphene_minecraft::EmbeddedInstallerInput>> {
    let mut unique = BTreeSet::new();
    let mut data_entries = BTreeMap::<String, String>::new();

    for (name, value) in data {
        collect_embedded_value(value, &mut unique)?;
        if let Some(entry) = client_embedded_entry(value) {
            data_entries.insert(name.clone(), entry.to_owned());
        }
    }

    let mut consumers = BTreeMap::<String, BTreeSet<String>>::new();
    for processor in processors {
        let mut names = BTreeSet::new();
        for argument in &processor.arguments {
            collect_argument_data_names(argument, &mut names)?;
        }

        for name in names {
            if let Some(entry) = data_entries.get(&name) {
                consumers
                    .entry(entry.clone())
                    .or_default()
                    .insert(processor.id.clone());
            }
        }
    }

    unique
        .into_iter()
        .map(|entry| {
            if !entries.contains(entry.as_str()) {
                return Err(loader_error(
                    ErrorCode::LoaderInstallerInvalid,
                    "loader embedded input is missing from the verified installer",
                ));
            }

            Ok(graphene_minecraft::EmbeddedInstallerInput {
                staging_path: ManagedPath::new(format!("inputs/{entry}"))?,
                entry: entry.clone(),
                expected_integrity: ArtifactIntegrity::none(),
                maximum_size: jar::MAX_INSTALLER_ENTRY_BYTES as u64,
                consumers: consumers
                    .remove(&entry)
                    .unwrap_or_default()
                    .into_iter()
                    .collect(),
            })
        })
        .collect()
}

fn client_embedded_entry(value: &PreparationDataValue) -> Option<&str> {
    match value {
        PreparationDataValue::EmbeddedInstallerEntry(value) => Some(value.as_str()),
        PreparationDataValue::SideSpecific { client, .. } => client_embedded_entry(client),
        _ => None,
    }
}

fn collect_argument_data_names(
    argument: &graphene_minecraft::PreparationArgument,
    output: &mut BTreeSet<String>,
) -> Result<()> {
    use graphene_minecraft::{
        PreparationArgument, PreparationArgumentPart, PreparationPlaceholder,
    };

    let mut visit = |value: &PreparationPlaceholder| {
        if let PreparationPlaceholder::Data(name) = value {
            output.insert(name.clone());
        }
    };

    match argument {
        PreparationArgument::Placeholder(value) => visit(value),
        PreparationArgument::Template(parts) => {
            for part in parts {
                if let PreparationArgumentPart::Placeholder(value) = part {
                    visit(value);
                }
            }
        }
        PreparationArgument::Literal(_) => {}
        _ => {
            return Err(loader_error(
                ErrorCode::LoaderProcessorUnsupported,
                "loader processor argument variant is unsupported",
            ));
        }
    }

    Ok(())
}

fn collect_embedded_value(
    value: &PreparationDataValue,
    output: &mut BTreeSet<String>,
) -> Result<()> {
    match value {
        PreparationDataValue::EmbeddedInstallerEntry(value) => {
            output.insert(value.clone());
        }
        PreparationDataValue::SideSpecific { client, .. } => {
            collect_embedded_value(client, output)?;
        }
        _ => {}
    }

    Ok(())
}
