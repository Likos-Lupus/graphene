use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    process::Command,
};

#[test]
fn phase0_dependency_architecture_is_enforced() {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .expect("cargo metadata must execute");
    assert!(output.status.success(), "cargo metadata failed");
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("metadata JSON");
    let packages = metadata["packages"].as_array().expect("packages array");
    let phase0 = [
        "graphene",
        "graphene-core",
        "graphene-platform",
        "graphene-network",
        "graphene-storage",
        "graphene-service",
    ];
    let phase0_set: HashSet<_> = phase0.into_iter().collect();
    let mut dependencies: HashMap<String, HashSet<String>> = HashMap::new();

    for package in packages {
        let name = package["name"].as_str().expect("package name");
        if !phase0_set.contains(name) {
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
        dependencies.insert(name.to_owned(), deps);
    }

    let forbidden = [
        ("graphene-core", "graphene-network"),
        ("graphene-core", "graphene-storage"),
        ("graphene-platform", "graphene-network"),
        ("graphene-network", "graphene-service"),
        ("graphene-network", "graphene-storage"),
        ("graphene-storage", "graphene-service"),
    ];
    for (from, to) in forbidden {
        assert!(
            !dependencies.get(from).is_some_and(|deps| deps.contains(to)),
            "forbidden dependency edge: {from} -> {to}"
        );
    }

    let expected_internal = HashMap::from([
        (
            "graphene",
            HashSet::from(["graphene-core", "graphene-service"]),
        ),
        ("graphene-core", HashSet::new()),
        ("graphene-platform", HashSet::from(["graphene-core"])),
        ("graphene-network", HashSet::from(["graphene-core"])),
        (
            "graphene-storage",
            HashSet::from(["graphene-core", "graphene-platform"]),
        ),
        (
            "graphene-service",
            HashSet::from([
                "graphene-core",
                "graphene-platform",
                "graphene-network",
                "graphene-storage",
            ]),
        ),
    ]);
    for (package, expected) in expected_internal {
        let actual = dependencies
            .get(package)
            .expect("Phase 0 package")
            .iter()
            .filter(|dependency| phase0_set.contains(dependency.as_str()))
            .map(String::as_str)
            .collect::<HashSet<_>>();
        assert_eq!(actual, expected, "unexpected internal edges for {package}");
    }

    let core_allowed = ["serde", "uuid"].into_iter().collect::<HashSet<_>>();
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
