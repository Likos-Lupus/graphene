#!/usr/bin/env python3
"""Mechanical Cargo and source-boundary architecture guard."""

from __future__ import annotations

import pathlib
import re
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
PACKAGE_NAMES = [
    "graphene",
    "graphene-core",
    "graphene-platform",
    "graphene-network",
    "graphene-storage",
    "graphene-minecraft",
    "graphene-instance",
    "graphene-java",
    "graphene-auth",
    "graphene-content",
    "graphene-providers",
    "graphene-modpack",
    "graphene-install",
    "graphene-launch",
    "graphene-service",
]
PACKAGES = {
    name: ROOT / ("Cargo.toml" if name == "graphene" else f"crates/{name}/Cargo.toml")
    for name in PACKAGE_NAMES
}

UI_DEPENDENCIES = {"tauri", "slint"}
CORE_ALLOWED = {"serde", "uuid"}
EXPLICIT_FORBIDDEN = {
    ("graphene-auth", "graphene-network"),
    ("graphene-auth", "graphene-storage"),
    ("graphene-auth", "graphene-service"),
    ("graphene-auth", "graphene-launch"),
    ("graphene-java", "graphene-providers"),
    ("graphene-java", "graphene-network"),
    ("graphene-java", "graphene-storage"),
    ("graphene-java", "graphene-service"),
    ("graphene-launch", "graphene-auth"),
    ("graphene-launch", "graphene-providers"),
    ("graphene-launch", "graphene-service"),
    ("graphene-minecraft", "graphene-network"),
    ("graphene-minecraft", "graphene-platform"),
    ("graphene-minecraft", "graphene-providers"),
    ("graphene-minecraft", "graphene-service"),
    ("graphene-minecraft", "graphene-install"),
    ("graphene-install", "graphene-providers"),
    ("graphene-install", "graphene-network"),
    ("graphene-install", "graphene-service"),
    ("graphene-install", "graphene-content"),
    ("graphene-providers", "graphene-service"),
    ("graphene-content", "graphene-providers"),
    ("graphene-content", "graphene-network"),
    ("graphene-content", "graphene-service"),
    ("graphene-launch", "graphene-content"),
    ("graphene-minecraft", "graphene-content"),
    ("graphene-auth", "graphene-content"),
    ("graphene-java", "graphene-content"),
    ("graphene-modpack", "graphene-service"),
    ("graphene-modpack", "graphene-network"),
    ("graphene-modpack", "graphene-storage"),
    ("graphene-modpack", "graphene-platform"),
    ("graphene-modpack", "graphene-providers"),
    ("graphene-modpack", "graphene-launch"),
    ("graphene-install", "graphene-modpack"),
    ("graphene-content", "graphene-modpack"),
    ("graphene-instance", "graphene-modpack"),
    ("graphene-launch", "graphene-modpack"),
}

MAX_CRATE_ROOT_LINES = 120
BUSINESS_ITEM_RE = re.compile(
    r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:fn|struct|enum|trait|impl)\b"
)
DTO_TYPE_RE = re.compile(r"\b[A-Za-z_]\w*Dto\b")
PUBLIC_DTO_RE = re.compile(
    r"(?m)^\s*(?:pub\s+|pub\(crate\)\s+)(?:struct|enum|type)\s+\w*Dto\b"
)


def dependencies(
    manifest: pathlib.Path,
    sections: tuple[str, ...] = ("dependencies", "build-dependencies"),
) -> set[str]:
    with manifest.open("rb") as handle:
        document = tomllib.load(handle)
    names: set[str] = set()
    for section in sections:
        names.update(document.get(section, {}).keys())
    for target_table in document.get("target", {}).values():
        for section in sections:
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
    for source_root in roots:
        if source_root.exists():
            result.extend(source_root.rglob("*.rs"))
    return result


def crate_root(package: str) -> pathlib.Path:
    return ROOT / ("src/lib.rs" if package == "graphene" else f"crates/{package}/src/lib.rs")


def main() -> None:
    missing = [str(path.relative_to(ROOT)) for path in PACKAGES.values() if not path.is_file()]
    if missing:
        fail(f"required workspace manifests are missing: {missing}")

    graph = {name: dependencies(path) for name, path in PACKAGES.items()}
    internal = set(PACKAGES)

    for package, path in PACKAGES.items():
        forbidden_ui = dependencies(
            path, ("dependencies", "dev-dependencies", "build-dependencies")
        ) & UI_DEPENDENCIES
        if forbidden_ui:
            fail(f"{package} has UI dependency {sorted(forbidden_ui)}")
    for package, deps in graph.items():
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
            relative = source_path.relative_to(ROOT)
            if package != "graphene-network" and re.search(r"\breqwest\s*::", text):
                fail(f"reqwest implementation type referenced by {relative}")
            if package not in {"graphene-providers", "graphene-modpack"} and DTO_TYPE_RE.search(text):
                fail(f"provider DTO-like type leaked into {relative}")
            if package == "graphene-providers" and PUBLIC_DTO_RE.search(text):
                fail(f"provider DTO publicly exported from {relative}")
            if package == "graphene-service" and re.search(
                r"graphene_providers::.*(?:fabric|forge|neoforge).*dto", text, re.DOTALL
            ):
                fail(f"service imports a private loader DTO module in {relative}")
            if package in {"graphene-minecraft", "graphene-install"} and re.search(
                r"\b(?:sh|bash|cmd\.exe|powershell)\b.*(?:-c|/C|-Command)", text, re.IGNORECASE
            ):
                fail(f"domain/install source contains a shell execution pattern in {relative}")


    duplicate_archive_codecs = [
        ROOT / "crates/graphene-install/src/archive/deflate.rs",
        ROOT / "crates/graphene-install/src/archive/zip.rs",
        ROOT / "crates/graphene-providers/src/loader/common/jar/deflate.rs",
        ROOT / "crates/graphene-providers/src/loader/common/jar/zip.rs",
    ]
    duplicated = [str(path.relative_to(ROOT)) for path in duplicate_archive_codecs if path.exists()]
    if duplicated:
        fail(
            "bounded ZIP/DEFLATE codec must remain centralized in graphene-core; "
            f"duplicate implementations found: {duplicated}"
        )

    for package in PACKAGES:
        facade = crate_root(package)
        if not facade.is_file():
            fail(f"crate root is missing for {package}")
        facade_text = facade.read_text(encoding="utf-8")
        line_count = len(facade_text.splitlines())
        if line_count > MAX_CRATE_ROOT_LINES:
            fail(
                f"{package} crate root has {line_count} lines; "
                f"crate roots must remain thin facades (max {MAX_CRATE_ROOT_LINES})"
            )
        if BUSINESS_ITEM_RE.search(facade_text):
            fail(f"{package} crate root defines business items instead of acting as a facade")

    print("Graphene architecture manifest/source checks passed.")


if __name__ == "__main__":
    main()
