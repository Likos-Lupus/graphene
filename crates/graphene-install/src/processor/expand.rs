use crate::error::install_error;
use graphene_core::{ErrorCode, Result};
use graphene_minecraft::{
    PreparationArgument, PreparationArgumentPart, PreparationDataValue, PreparationPlaceholder,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub const MAX_PROCESSOR_ARGUMENTS: usize = 4096;
pub const MAX_PROCESSOR_ARGUMENT_BYTES: usize = 16 * 1024;

#[derive(Debug)]
pub struct ProcessorExpansionContext<'a> {
    pub staging_root: &'a Path,
    pub root: &'a Path,
    pub installer: &'a Path,
    pub library_dir: &'a Path,
    pub minecraft_jar: &'a Path,
    pub minecraft_version: &'a str,
    pub data: &'a BTreeMap<String, PreparationDataValue>,
}

pub fn expand_processor_arguments(
    arguments: &[PreparationArgument],
    context: &ProcessorExpansionContext<'_>,
) -> Result<Vec<String>> {
    if arguments.len() > MAX_PROCESSOR_ARGUMENTS {
        return Err(install_error(
            ErrorCode::LoaderProcessorPlaceholderInvalid,
            "loader processor contains too many arguments",
        ));
    }
    arguments
        .iter()
        .map(|argument| expand_argument(argument, context))
        .collect()
}

fn expand_argument(
    argument: &PreparationArgument,
    context: &ProcessorExpansionContext<'_>,
) -> Result<String> {
    let value = match argument {
        PreparationArgument::Literal(value) => value.clone(),
        PreparationArgument::Placeholder(value) => expand_placeholder(value, context)?,
        PreparationArgument::Template(parts) => {
            if parts.len() > 128 {
                return Err(invalid(
                    "loader processor argument template contains too many parts",
                ));
            }

            let mut output = String::new();
            for part in parts {
                match part {
                    PreparationArgumentPart::Literal(value) => output.push_str(value),
                    PreparationArgumentPart::Placeholder(value) => {
                        output.push_str(&expand_placeholder(value, context)?)
                    }
                    _ => {
                        return Err(invalid(
                            "loader processor argument template part is unsupported",
                        ));
                    }
                }

                if output.len() > MAX_PROCESSOR_ARGUMENT_BYTES {
                    return Err(invalid("loader processor argument exceeds the size limit"));
                }
            }
            output
        }
        _ => return Err(invalid("loader processor argument variant is unsupported")),
    };

    if value.len() > MAX_PROCESSOR_ARGUMENT_BYTES || value.contains('\0') {
        return Err(invalid("loader processor argument is invalid"));
    }

    Ok(value)
}

fn expand_placeholder(
    value: &PreparationPlaceholder,
    context: &ProcessorExpansionContext<'_>,
) -> Result<String> {
    match value {
        PreparationPlaceholder::Root => safe_path(context.root, context.staging_root),
        PreparationPlaceholder::Installer => safe_path(context.installer, context.staging_root),
        PreparationPlaceholder::LibraryDir => safe_path(context.library_dir, context.staging_root),
        PreparationPlaceholder::MinecraftJar => {
            safe_path(context.minecraft_jar, context.staging_root)
        }
        PreparationPlaceholder::MinecraftVersion => Ok(context.minecraft_version.to_owned()),
        PreparationPlaceholder::Side => Ok("client".to_owned()),
        PreparationPlaceholder::MavenPath(coordinate) => {
            let relative = coordinate.repository_path()?;
            safe_path(
                &context.library_dir.join(relative.as_str()),
                context.staging_root,
            )
        }
        PreparationPlaceholder::Data(name) => {
            if name.is_empty() || name.len() > 128 || name.contains('\0') {
                return Err(invalid("loader processor data placeholder is invalid"));
            }
            let value = context
                .data
                .get(name)
                .ok_or_else(|| invalid("loader processor data placeholder is missing"))?;
            expand_data(value, context)
        }
        _ => Err(invalid("loader processor placeholder is unsupported")),
    }
}

fn expand_data(
    value: &PreparationDataValue,
    context: &ProcessorExpansionContext<'_>,
) -> Result<String> {
    match value {
        PreparationDataValue::Literal(value) => {
            if value.contains('\0') || value.len() > MAX_PROCESSOR_ARGUMENT_BYTES {
                Err(invalid("loader processor data literal is invalid"))
            } else {
                Ok(value.clone())
            }
        }
        PreparationDataValue::MavenCoordinate(coordinate) => {
            let relative = coordinate.repository_path()?;
            safe_path(
                &context.library_dir.join(relative.as_str()),
                context.staging_root,
            )
        }
        PreparationDataValue::EmbeddedInstallerEntry(value) => {
            if value.starts_with('/')
                || value.starts_with('\\')
                || value.contains("..")
                || value.contains('\\')
                || value.contains('\0')
            {
                return Err(invalid("loader embedded data path is invalid"));
            }
            safe_path(
                &context.staging_root.join("inputs").join(value),
                context.staging_root,
            )
        }
        PreparationDataValue::ManagedPath(value) => safe_path(
            &context.staging_root.join(value.as_str()),
            context.staging_root,
        ),
        PreparationDataValue::SideSpecific { client, .. } => expand_data(client, context),
        _ => Err(invalid("loader processor data value is unsupported")),
    }
}

fn safe_path(path: &Path, root: &Path) -> Result<String> {
    let normalized = lexical_normalize(path)?;
    let normalized_root = lexical_normalize(root)?;
    if !normalized.starts_with(&normalized_root) {
        return Err(invalid("loader processor path expansion escapes staging"));
    }

    normalized
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("loader processor path is not UTF-8"))
}

fn lexical_normalize(path: &Path) -> Result<PathBuf> {
    use std::path::Component;
    let mut output = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => output.push(prefix.as_os_str()),
            Component::RootDir => output.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !output.pop() {
                    return Err(invalid("loader processor path contains traversal"));
                }
            }
            Component::Normal(value) => output.push(value),
        }
    }
    Ok(output)
}

fn invalid(message: &'static str) -> graphene_core::GrapheneError {
    install_error(ErrorCode::LoaderProcessorPlaceholderInvalid, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphene_minecraft::{MavenCoordinate, PreparationArgumentPart};

    #[test]
    fn known_placeholders_expand_without_shell_quoting() {
        let root = if cfg!(windows) {
            PathBuf::from(r"C:\graphene stage")
        } else {
            PathBuf::from("/tmp/graphene stage")
        };

        let context = ProcessorExpansionContext {
            staging_root: &root,
            installer: &root.join("installer/installer.jar"),
            library_dir: &root.join("root/libraries"),
            minecraft_jar: &root.join("root/versions/1.21.1/client.jar"),
            root: &root,
            minecraft_version: "1.21.1",
            data: &BTreeMap::new(),
        };
        let args = vec![
            PreparationArgument::Placeholder(PreparationPlaceholder::Root),
            PreparationArgument::Placeholder(PreparationPlaceholder::Side),
            PreparationArgument::Template(vec![
                PreparationArgumentPart::Literal("--lib=".into()),
                PreparationArgumentPart::Placeholder(PreparationPlaceholder::MavenPath(
                    MavenCoordinate::parse("org.example:demo:1").expect("coordinate"),
                )),
            ]),
        ];
        let expanded = expand_processor_arguments(&args, &context).expect("expand");

        assert_eq!(expanded[1], "client");
        assert!(expanded[0].contains("graphene stage"));
        let library = expanded[2].strip_prefix("--lib=").expect("template prefix");
        let expected_suffix = Path::new("org")
            .join("example")
            .join("demo")
            .join("1")
            .join("demo-1.jar");
        assert!(Path::new(library).ends_with(expected_suffix));
    }
}

#[cfg(test)]
mod phase3_matrix_tests {
    use super::*;
    use graphene_minecraft::{ManagedPath, MavenCoordinate};

    fn with_context<T>(test: impl FnOnce(&ProcessorExpansionContext<'_>) -> T) -> T {
        let staging = if cfg!(windows) {
            PathBuf::from(r"C:\Graphene Test\staging")
        } else {
            PathBuf::from("/tmp/graphene-test/staging")
        };

        let root = staging.join("root");
        let installer = staging.join("installer/installer.jar");
        let libraries = root.join("libraries");
        let minecraft = root.join("versions/1.21.1/client.jar");
        let mut data = BTreeMap::new();

        data.insert(
            "LITERAL".to_owned(),
            PreparationDataValue::Literal("value".to_owned()),
        );
        data.insert(
            "MAVEN".to_owned(),
            PreparationDataValue::MavenCoordinate(
                MavenCoordinate::parse("org.example:demo:1").expect("coordinate"),
            ),
        );
        data.insert(
            "EMBEDDED".to_owned(),
            PreparationDataValue::EmbeddedInstallerEntry("data/client.lzma".to_owned()),
        );
        data.insert(
            "MANAGED".to_owned(),
            PreparationDataValue::ManagedPath(
                ManagedPath::new("outputs/result.jar").expect("path"),
            ),
        );

        let context = ProcessorExpansionContext {
            staging_root: &staging,
            root: &root,
            installer: &installer,
            library_dir: &libraries,
            minecraft_jar: &minecraft,
            minecraft_version: "1.21.1",
            data: &data,
        };

        test(&context)
    }

    #[test]
    fn every_phase3_placeholder_category_expands_explicitly() {
        with_context(|context| {
            let args = vec![
                PreparationArgument::Literal("literal".to_owned()),
                PreparationArgument::Placeholder(PreparationPlaceholder::Root),
                PreparationArgument::Placeholder(PreparationPlaceholder::Installer),
                PreparationArgument::Placeholder(PreparationPlaceholder::LibraryDir),
                PreparationArgument::Placeholder(PreparationPlaceholder::MinecraftJar),
                PreparationArgument::Placeholder(PreparationPlaceholder::MinecraftVersion),
                PreparationArgument::Placeholder(PreparationPlaceholder::Side),
                PreparationArgument::Placeholder(PreparationPlaceholder::Data(
                    "LITERAL".to_owned(),
                )),
                PreparationArgument::Placeholder(PreparationPlaceholder::Data("MAVEN".to_owned())),
                PreparationArgument::Placeholder(PreparationPlaceholder::Data(
                    "EMBEDDED".to_owned(),
                )),
                PreparationArgument::Placeholder(PreparationPlaceholder::Data(
                    "MANAGED".to_owned(),
                )),
            ];
            let expanded = expand_processor_arguments(&args, context).expect("expand");
            assert_eq!(expanded[0], "literal");
            assert_eq!(expanded[5], "1.21.1");
            assert_eq!(expanded[6], "client");
            assert_eq!(expanded[7], "value");
            assert!(
                expanded[8].ends_with("org/example/demo/1/demo-1.jar")
                    || expanded[8].ends_with(r"org\example\demo\1\demo-1.jar")
            );
            assert!(expanded[9].contains("client.lzma"));
            assert!(expanded[10].contains("result.jar"));
        });
    }

    #[test]
    fn missing_data_nul_and_argument_bounds_fail() {
        with_context(|context| {
            let missing = [PreparationArgument::Placeholder(
                PreparationPlaceholder::Data("MISSING".to_owned()),
            )];
            assert_eq!(
                expand_processor_arguments(&missing, context)
                    .expect_err("missing")
                    .code,
                ErrorCode::LoaderProcessorPlaceholderInvalid
            );

            let nul = [PreparationArgument::Literal("bad\0arg".to_owned())];
            assert!(expand_processor_arguments(&nul, context).is_err());

            let long = [PreparationArgument::Literal(
                "x".repeat(MAX_PROCESSOR_ARGUMENT_BYTES + 1),
            )];
            assert!(expand_processor_arguments(&long, context).is_err());

            let too_many =
                vec![PreparationArgument::Literal("x".to_owned()); MAX_PROCESSOR_ARGUMENTS + 1];
            assert!(expand_processor_arguments(&too_many, context).is_err());
        });
    }

    #[test]
    fn embedded_and_managed_paths_cannot_escape_staging() {
        let staging = PathBuf::from("/managed/staging");
        assert!(safe_path(&PathBuf::from("/managed/other/file"), &staging).is_err());
        assert!(lexical_normalize(Path::new("../../escape")).is_err());
    }
}
