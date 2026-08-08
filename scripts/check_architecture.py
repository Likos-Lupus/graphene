#!/usr/bin/env python3
"""Mechanical Phase 0/Phase 1 Cargo and source-boundary architecture guard."""

from __future__ import annotations

import pathlib
import re
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
PACKAGES = {
    "graphene": ROOT / "Cargo.toml",
    "graphene-core": ROOT / "crates/graphene-core/Cargo.toml",
    "graphene-platform": ROOT / "crates/graphene-platform/Cargo.toml",
    "graphene-network": ROOT / "crates/graphene-network/Cargo.toml",
    "graphene-storage": ROOT / "crates/graphene-storage/Cargo.toml",
    "graphene-minecraft": ROOT / "crates/graphene-minecraft/Cargo.toml",
    "graphene-instance": ROOT / "crates/graphene-instance/Cargo.toml",
    "graphene-java": ROOT / "crates/graphene-java/Cargo.toml",
    "graphene-providers": ROOT / "crates/graphene-providers/Cargo.toml",
    "graphene-install": ROOT / "crates/graphene-install/Cargo.toml",
    "graphene-launch": ROOT / "crates/graphene-launch/Cargo.toml",
    "graphene-service": ROOT / "crates/graphene-service/Cargo.toml",
}

UI_DEPENDENCIES = {"tauri", "slint"}
CORE_ALLOWED = {"serde", "uuid"}

EXPECTED_INTERNAL_GRAPH = {
    "graphene": {
        "graphene-core",
        "graphene-minecraft",
        "graphene-instance",
        "graphene-java",
        "graphene-install",
        "graphene-launch",
        "graphene-service",
    },
    "graphene-core": set(),
    "graphene-platform": {"graphene-core"},
    "graphene-network": {"graphene-core"},
    "graphene-storage": {"graphene-core", "graphene-platform"},
    "graphene-minecraft": {"graphene-core"},
    "graphene-instance": {"graphene-core"},
    "graphene-java": {"graphene-core", "graphene-platform"},
    "graphene-providers": {"graphene-core", "graphene-network", "graphene-minecraft"},
    "graphene-install": {
        "graphene-core",
        "graphene-minecraft",
        "graphene-instance",
        "graphene-storage",
        "graphene-platform",
    },
    "graphene-launch": {
        "graphene-core",
        "graphene-minecraft",
        "graphene-instance",
        "graphene-java",
        "graphene-platform",
    },
    "graphene-service": {
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
    },
}


PHASE1_FACADE_ROOTS = {
    "graphene-minecraft": ROOT / "crates/graphene-minecraft/src/lib.rs",
    "graphene-instance": ROOT / "crates/graphene-instance/src/lib.rs",
    "graphene-java": ROOT / "crates/graphene-java/src/lib.rs",
    "graphene-providers": ROOT / "crates/graphene-providers/src/lib.rs",
    "graphene-install": ROOT / "crates/graphene-install/src/lib.rs",
    "graphene-launch": ROOT / "crates/graphene-launch/src/lib.rs",
}
MAX_PHASE1_FACADE_LINES = 200

EXPLICIT_FORBIDDEN = {
    ("graphene-minecraft", "graphene-network"),
    ("graphene-minecraft", "graphene-platform"),
    ("graphene-install", "graphene-service"),
    ("graphene-launch", "graphene-service"),
    ("graphene-launch", "graphene-providers"),
    ("graphene-java", "graphene-service"),
    ("graphene-instance", "reqwest"),
    ("graphene-providers", "graphene-service"),
}


def dependencies(manifest: pathlib.Path) -> set[str]:
    with manifest.open("rb") as handle:
        document = tomllib.load(handle)
    names: set[str] = set()
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        names.update(document.get(section, {}).keys())
    for target_table in document.get("target", {}).values():
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            names.update(target_table.get(section, {}).keys())
    return names


def fail(message: str) -> None:
    raise SystemExit(f"architecture check failed: {message}")


def rust_sources(package: str) -> list[pathlib.Path]:
    if package == "graphene":
        roots = [ROOT / "src", ROOT / "tests"]
    else:
        roots = [ROOT / "crates" / package / "src", ROOT / "crates" / package / "tests"]
    result: list[pathlib.Path] = []
    for root in roots:
        if root.exists():
            result.extend(root.rglob("*.rs"))
    return result


def main() -> None:
    missing = [str(path.relative_to(ROOT)) for path in PACKAGES.values() if not path.is_file()]
    if missing:
        fail(f"required Phase 1 manifests are missing: {missing}")

    graph = {name: dependencies(path) for name, path in PACKAGES.items()}
    internal = set(PACKAGES)

    for package, deps in graph.items():
        forbidden_ui = deps & UI_DEPENDENCIES
        if forbidden_ui:
            fail(f"{package} has UI dependency {sorted(forbidden_ui)}")
        if package != "graphene-network" and "reqwest" in deps:
            fail(f"reqwest escaped network boundary into {package}")

    for source, destination in EXPLICIT_FORBIDDEN:
        if destination in graph[source]:
            fail(f"forbidden edge {source} -> {destination}")

    unexpected_core = graph["graphene-core"] - CORE_ALLOWED
    if unexpected_core:
        fail(f"graphene-core has infrastructure dependency {sorted(unexpected_core)}")

    internal_graph = {
        package: {dependency for dependency in deps if dependency in internal}
        for package, deps in graph.items()
    }
    for package, expected in EXPECTED_INTERNAL_GRAPH.items():
        if internal_graph[package] != expected:
            fail(
                f"{package} internal dependencies {sorted(internal_graph[package])} "
                f"do not match approved Phase 1 edges {sorted(expected)}"
            )

    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(package: str) -> None:
        if package in visited:
            return
        if package in visiting:
            fail(f"dependency cycle includes {package}")
        visiting.add(package)
        for dependency in internal_graph[package]:
            visit(dependency)
        visiting.remove(package)
        visited.add(package)

    for package in internal_graph:
        visit(package)

    for package in PACKAGES:
        for source_path in rust_sources(package):
            text = source_path.read_text(encoding="utf-8")
            if package != "graphene-network" and re.search(r"\breqwest\s*::", text):
                fail(f"reqwest implementation type referenced by {source_path.relative_to(ROOT)}")
            if package != "graphene-providers" and re.search(r"\b(?:Mojang|Manifest|Version|Asset)\w*Dto\b", text):
                fail(f"provider DTO-like type leaked into {source_path.relative_to(ROOT)}")
            if package == "graphene-providers":
                if re.search(r"(?m)^\s*pub\s+(?:struct|enum)\s+\w*Dto\b", text):
                    fail(f"provider DTO publicly exported from {source_path.relative_to(ROOT)}")
                if re.search(r"(?m)^\s*pub\(crate\)\s+(?:struct|enum)\s+\w*Dto\b", text):
                    fail(f"provider DTO escaped Mojang namespace in {source_path.relative_to(ROOT)}")

    for package, facade in PHASE1_FACADE_ROOTS.items():
        facade_text = facade.read_text(encoding="utf-8")
        line_count = len(facade_text.splitlines())
        if line_count > MAX_PHASE1_FACADE_LINES:
            fail(
                f"{package} crate root has {line_count} lines; "
                f"Phase 1 crate roots must remain thin facades (max {MAX_PHASE1_FACADE_LINES})"
            )
        if re.search(
            r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?"
            r"(?:fn|struct|enum|trait|impl)\b",
            facade_text,
        ):
            fail(f"{package} crate root defines business items instead of acting as a facade")

    print("Phase 1 architecture manifest/source checks passed.")


if __name__ == "__main__":
    main()
