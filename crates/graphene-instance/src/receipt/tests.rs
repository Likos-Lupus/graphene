use super::*;

fn artifact(path: &str) -> InstalledArtifact {
    InstalledArtifact {
        path: ManagedRelativePath::new(path).expect("managed path"),
        integrity: ArtifactIntegrity::none().with_sha1(
            "0123456789abcdef0123456789abcdef01234567"
                .parse()
                .expect("sha1"),
        ),
        expected_size: Some(1),
    }
}

fn current_receipt() -> InstallReceipt {
    InstallReceipt {
        schema_version: INSTALL_RECEIPT_SCHEMA_VERSION,
        install_format_version: INSTALL_FORMAT_VERSION,
        instance_id: InstanceId::new(),
        requested_version: "1.21.1".to_owned(),
        resolved_version: "1.21.1".to_owned(),
        components: vec![InstalledComponent {
            uid: "net.minecraft".to_owned(),
            version: "1.21.1".to_owned(),
            kind: InstalledComponentKind::Minecraft,
            provider: "mojang".to_owned(),
            provenance: Some("version-manifest".to_owned()),
        }],
        version_type: "release".to_owned(),
        main_class: "net.minecraft.client.main.Main".to_owned(),
        java_requirement: InstalledJavaRequirement {
            major_version: 21,
            component_hint: Some("java-runtime-delta".to_owned()),
        },
        client: artifact("shared/versions/1.21.1/client.jar"),
        libraries: Vec::new(),
        asset_index_id: "17".to_owned(),
        asset_index: artifact("shared/assets/indexes/17.json"),
        assets_root: ManagedRelativePath::new("shared/assets").expect("assets"),
        natives_directory: ManagedRelativePath::new("natives").expect("natives"),
        logging_configuration: None,
        logging_argument: None,
        jvm_arguments: Vec::new(),
        game_arguments: Vec::new(),
    }
}

#[test]
fn legacy_receipt_without_components_is_read_as_vanilla() {
    let mut value = serde_json::to_value(current_receipt()).expect("serialize");
    let object = value.as_object_mut().expect("receipt object");
    object.insert("schema_version".to_owned(), serde_json::json!(1));
    object.remove("components");
    let bytes = serde_json::to_vec(&value).expect("json");

    let migrated = InstallReceipt::from_json(&bytes).expect("legacy receipt");
    assert_eq!(migrated.schema_version, INSTALL_RECEIPT_SCHEMA_VERSION);
    assert_eq!(migrated.components.len(), 1);
    assert_eq!(migrated.components[0].uid, "net.minecraft");
    assert_eq!(migrated.components[0].version, "1.21.1");
    assert_eq!(migrated.components[0].provider, "mojang");
}

#[test]
fn malformed_legacy_receipt_with_components_is_rejected() {
    let mut receipt = current_receipt();
    receipt.schema_version = 1;
    let bytes = serde_json::to_vec(&receipt).expect("json");
    assert!(InstallReceipt::from_json(&bytes).is_err());
}

#[test]
fn loader_receipt_requires_exactly_one_base_and_at_most_one_primary_loader() {
    let mut receipt = current_receipt();

    receipt.components.push(InstalledComponent {
        uid: "net.fabricmc.fabric-loader".to_owned(),
        version: "0.16.10".to_owned(),
        kind: InstalledComponentKind::Loader,
        provider: "fabric-meta".to_owned(),
        provenance: Some("profile".to_owned()),
    });

    receipt.validate().expect("one loader");

    receipt.components.push(InstalledComponent {
        uid: "net.minecraftforge.forge".to_owned(),
        version: "52.0.1".to_owned(),
        kind: InstalledComponentKind::Loader,
        provider: "forge-maven".to_owned(),
        provenance: None,
    });

    assert!(receipt.validate().is_err());
}
