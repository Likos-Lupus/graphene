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

mod validation_matrix {
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
    fn all_supported_placeholder_categories_expand_explicitly() {
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
