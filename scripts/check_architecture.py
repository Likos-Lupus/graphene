#!/usr/bin/env python3
"""Lightweight Phase 0 Cargo-manifest architecture guard."""

from __future__ import annotations

import pathlib
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
PACKAGES = {
    "graphene": ROOT / "Cargo.toml",
    "graphene-core": ROOT / "crates/graphene-core/Cargo.toml",
    "graphene-platform": ROOT / "crates/graphene-platform/Cargo.toml",
    "graphene-network": ROOT / "crates/graphene-network/Cargo.toml",
    "graphene-storage": ROOT / "crates/graphene-storage/Cargo.toml",
    "graphene-service": ROOT / "crates/graphene-service/Cargo.toml",
}
FORBIDDEN = {
    ("graphene-core", "graphene-network"),
    ("graphene-core", "graphene-storage"),
    ("graphene-platform", "graphene-network"),
    ("graphene-network", "graphene-service"),
    ("graphene-network", "graphene-storage"),
    ("graphene-storage", "graphene-service"),
}
UI_DEPENDENCIES = {"tauri", "slint"}
CORE_ALLOWED = {"serde", "uuid"}


def dependencies(manifest: pathlib.Path) -> set[str]:
    with manifest.open("rb") as handle:
        document = tomllib.load(handle)
    names: set[str] = set()
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        names.update(document.get(section, {}).keys())
    target = document.get("target", {})
    for target_table in target.values():
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            names.update(target_table.get(section, {}).keys())
    return names


def fail(message: str) -> None:
    raise SystemExit(f"architecture check failed: {message}")


def main() -> None:
    graph = {name: dependencies(path) for name, path in PACKAGES.items()}
    phase0 = set(PACKAGES)

    for package, deps in graph.items():
        forbidden_ui = deps & UI_DEPENDENCIES
        if forbidden_ui:
            fail(f"{package} has UI dependency {sorted(forbidden_ui)}")

    for source, destination in FORBIDDEN:
        if destination in graph[source]:
            fail(f"forbidden edge {source} -> {destination}")

    unexpected_core = graph["graphene-core"] - CORE_ALLOWED
    if unexpected_core:
        fail(f"graphene-core has infrastructure dependency {sorted(unexpected_core)}")

    internal_graph = {
        package: {dep for dep in deps if dep in phase0}
        for package, deps in graph.items()
    }
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

    expected_internal_graph = {
        "graphene": {"graphene-core", "graphene-service"},
        "graphene-core": set(),
        "graphene-platform": {"graphene-core"},
        "graphene-network": {"graphene-core"},
        "graphene-storage": {"graphene-core", "graphene-platform"},
        "graphene-service": {
            "graphene-core",
            "graphene-platform",
            "graphene-network",
            "graphene-storage",
        },
    }
    for package, expected in expected_internal_graph.items():
        if internal_graph[package] != expected:
            fail(
                f"{package} internal dependencies {sorted(internal_graph[package])} "
                f"do not match approved Phase 0 edges {sorted(expected)}"
            )

    print("Phase 0 architecture manifest checks passed.")


if __name__ == "__main__":
    main()
