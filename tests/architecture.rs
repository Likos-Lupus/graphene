use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    process::Command,
};

#[test]
fn phase1_dependency_architecture_is_enforced() {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .expect("cargo metadata must execute");
    assert!(output.status.success(), "cargo metadata failed");
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("metadata JSON");
    let packages = metadata["packages"].as_array().expect("packages array");
    let phase1 = [
        "graphene",
        "graphene-core",
        "graphene-platform",
        "graphene-network",
        "graphene-storage",
        "graphene-minecraft",
        "graphene-instance",
        "graphene-java",
        "graphene-providers",
        "graphene-install",
        "graphene-launch",
        "graphene-service",
    ];
    let phase1_set: HashSet<_> = phase1.into_iter().collect();
    let mut dependencies: HashMap<String, HashSet<String>> = HashMap::new();
    for package in packages {
        let name = package["name"].as_str().expect("package name");
        if !phase1_set.contains(name) {
            continue;
        }
        let deps = package["dependencies"]
            .as_array()
            .expect("dependencies")
            .iter()
            .filter_map(|dependency| dependency["name"].as_str())
            .map(str::to_owned)
            .collect::<HashSet<_>>();
        assert!(!deps.contains("tauri"), "{name} must not depend on tauri");
        assert!(!deps.contains("slint"), "{name} must not depend on slint");
        if name != "graphene-network" {
            assert!(
                !deps.contains("reqwest"),
                "reqwest escaped network boundary into {name}"
            );
        }
        dependencies.insert(name.to_owned(), deps);
    }
    let expected = HashMap::from([
        (
            "graphene",
            HashSet::from([
                "graphene-core",
                "graphene-minecraft",
                "graphene-instance",
                "graphene-java",
                "graphene-install",
                "graphene-launch",
                "graphene-service",
            ]),
        ),
        ("graphene-core", HashSet::new()),
        ("graphene-platform", HashSet::from(["graphene-core"])),
        ("graphene-network", HashSet::from(["graphene-core"])),
        (
            "graphene-storage",
            HashSet::from(["graphene-core", "graphene-platform"]),
        ),
        ("graphene-minecraft", HashSet::from(["graphene-core"])),
        ("graphene-instance", HashSet::from(["graphene-core"])),
        (
            "graphene-java",
            HashSet::from(["graphene-core", "graphene-platform"]),
        ),
        (
            "graphene-providers",
            HashSet::from(["graphene-core", "graphene-network", "graphene-minecraft"]),
        ),
        (
            "graphene-install",
            HashSet::from([
                "graphene-core",
                "graphene-minecraft",
                "graphene-instance",
                "graphene-storage",
                "graphene-platform",
            ]),
        ),
        (
            "graphene-launch",
            HashSet::from([
                "graphene-core",
                "graphene-minecraft",
                "graphene-instance",
                "graphene-java",
                "graphene-platform",
            ]),
        ),
        (
            "graphene-service",
            HashSet::from([
                "graphene-core",
                "graphene-platform",
                "graphene-network",
                "graphene-storage",
                "graphene-minecraft",
                "graphene-instance",
                "graphene-java",
                "graphene-providers",
                "graphene-install",
                "graphene-launch",
            ]),
        ),
    ]);
    for (package, expected_internal) in expected {
        let actual = dependencies
            .get(package)
            .expect("Phase 1 package")
            .iter()
            .filter(|dependency| phase1_set.contains(dependency.as_str()))
            .map(String::as_str)
            .collect::<HashSet<_>>();
        assert_eq!(
            actual, expected_internal,
            "unexpected internal edges for {package}"
        );
    }
    let core_allowed = HashSet::from(["serde", "uuid"]);
    for dependency in dependencies.get("graphene-core").expect("core package") {
        assert!(
            core_allowed.contains(dependency.as_str()),
            "graphene-core acquired infrastructure dependency {dependency}"
        );
    }
    fn visit(
        node: &str,
        dependencies: &HashMap<String, HashSet<String>>,
        visiting: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) {
        if visited.contains(node) {
            return;
        }
        assert!(
            visiting.insert(node.to_owned()),
            "dependency cycle detected at {node}"
        );
        if let Some(edges) = dependencies.get(node) {
            for edge in edges {
                if dependencies.contains_key(edge) {
                    visit(edge, dependencies, visiting, visited);
                }
            }
        }
        visiting.remove(node);
        visited.insert(node.to_owned());
    }
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for node in dependencies.keys() {
        visit(node, &dependencies, &mut visiting, &mut visited);
    }
}

#[test]
fn phase1_documented_root_facade_imports_compile() {
    use graphene::{
        ArtifactAcquirer, Graphene, InstallPlan, InstallRequest, JavaRuntime, LaunchPlan,
        LaunchRequest, LaunchSession, MinecraftVersionId, MojangProviderConfig, NewInstanceSpec,
        RedactedLaunchPlan, ResolvedMinecraft, SensitiveString, VersionManifest,
    };

    fn assert_type<T: 'static>() {
        let _ = std::any::TypeId::of::<T>();
    }

    assert_type::<Graphene>();
    assert_type::<InstallPlan>();
    assert_type::<InstallRequest>();
    assert_type::<JavaRuntime>();
    assert_type::<LaunchPlan>();
    assert_type::<LaunchRequest>();
    assert_type::<LaunchSession>();
    assert_type::<MinecraftVersionId>();
    assert_type::<MojangProviderConfig>();
    assert_type::<NewInstanceSpec>();
    assert_type::<RedactedLaunchPlan>();
    assert_type::<ResolvedMinecraft>();
    assert_type::<SensitiveString>();
    assert_type::<VersionManifest>();
    let _: Option<&dyn ArtifactAcquirer> = None;
}
