#!/usr/bin/env python3
"""
Mechanical Cargo and source-boundary architecture guard.

Engine packages live in the root workspace and are strictly UI-free. Reference host packages live
in the separate `apps/` workspace and consume only the root `graphene` facade. This script keeps the
two populations explicit instead of weakening the engine ban globally.
"""

from __future__ import annotations

import pathlib
import re
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]

ENGINE_PACKAGE_NAMES = [
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
    "graphene-diagnostics",
]
HOST_MANIFEST_PATHS = {
    "graphene-reference-host-support": "apps/graphene-reference-host-support/Cargo.toml",
    "graphene-cli": "apps/graphene-cli/Cargo.toml",
    "graphene-tauri-host": "apps/graphene-tauri/src-tauri/Cargo.toml",
}
TAURI_HOST = "graphene-tauri-host"

ENGINE_PACKAGES = {
    name: ROOT / ("Cargo.toml" if name == "graphene" else f"crates/{name}/Cargo.toml")
    for name in ENGINE_PACKAGE_NAMES
}
HOST_PACKAGES = {name: ROOT / path for name, path in HOST_MANIFEST_PATHS.items()}
ALL_PACKAGES = {**ENGINE_PACKAGES, **HOST_PACKAGES}
ENGINE_INTERNAL = set(ENGINE_PACKAGES)
HOST_INTERNAL = set(HOST_PACKAGES)

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
    # Diagnostics analyzes normalized snapshots only; it never becomes a storage, orchestration,
    # network, provider, auth, or host layer.
    ("graphene-diagnostics", "graphene-service"),
    ("graphene-diagnostics", "graphene-providers"),
    ("graphene-diagnostics", "graphene-network"),
    ("graphene-diagnostics", "graphene-storage"),
    ("graphene-diagnostics", "graphene-auth"),
    ("graphene-diagnostics", "graphene-launch"),
    ("graphene-diagnostics", "graphene-modpack"),
    ("graphene-diagnostics", "graphene-install"),
    ("graphene-diagnostics", "graphene-minecraft"),
    ("graphene-diagnostics", "graphene-platform"),
    ("graphene-diagnostics", "graphene-java"),
    # Upstream bounded contexts must not gain a reverse dependency on diagnostics merely to emit
    # findings; existing low-level diagnostics remain legal inputs to the aggregator.
    ("graphene-instance", "graphene-diagnostics"),
    ("graphene-content", "graphene-diagnostics"),
    ("graphene-java", "graphene-diagnostics"),
    ("graphene-launch", "graphene-diagnostics"),
}

MAX_CRATE_ROOT_LINES = 120
MAX_HOST_MAIN_LINES = 60
BUSINESS_ITEM_RE = re.compile(
    r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:fn|struct|enum|trait|impl)\b",
)
DTO_TYPE_RE = re.compile(r"\b[A-Za-z_]\w*Dto\b")
PUBLIC_DTO_RE = re.compile(
    r"(?m)^\s*(?:pub\s+|pub\(crate\)\s+)(?:struct|enum|type)\s+\w*Dto\b",
)
ENGINE_NAMESPACE_RE = re.compile(
    r"\bgraphene_(?:core|platform|network|storage|minecraft|instance|java|auth|content|"
    r"providers|modpack|install|launch|service|diagnostics)\b",
)
EXCLUDED_SOURCE_DIRS = {"node_modules", "dist", "target", "gen", ".vite"}


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


def workspace_members(manifest: pathlib.Path) -> list[str]:
    with manifest.open("rb") as handle:
        document = tomllib.load(handle)
    return list(document.get("workspace", {}).get("members", []))


def fail(message: str) -> None:
    raise SystemExit(f"architecture check failed: {message}")


def package_dir(package: str) -> pathlib.Path:
    if package in HOST_PACKAGES:
        return HOST_PACKAGES[package].parent
    if package == "graphene":
        return ROOT
    return ROOT / "crates" / package


def rust_sources(package: str) -> list[pathlib.Path]:
    base = package_dir(package)
    roots = [base / "src", base / "tests"]
    result: list[pathlib.Path] = []
    for source_root in roots:
        if not source_root.exists():
            continue
        for path in source_root.rglob("*.rs"):
            if EXCLUDED_SOURCE_DIRS.intersection(path.relative_to(base).parts):
                continue
            result.append(path)
    return result


def crate_root(package: str) -> pathlib.Path:
    return package_dir(package) / "src" / "lib.rs"


def main() -> None:
    missing = [str(path.relative_to(ROOT)) for path in ALL_PACKAGES.values() if not path.is_file()]
    if missing:
        fail(f"required workspace manifests are missing: {missing}")

    graph = {name: dependencies(path) for name, path in ENGINE_PACKAGES.items()}
    host_graph = {name: dependencies(path) for name, path in HOST_PACKAGES.items()}

    # Engine UI ban is unchanged and absolute.
    for package, path in ENGINE_PACKAGES.items():
        forbidden_ui = dependencies(
            path, ("dependencies", "dev-dependencies", "build-dependencies"),
        ) & UI_DEPENDENCIES
        if forbidden_ui:
            fail(f"engine package {package} has UI dependency {sorted(forbidden_ui)}")

    # Host UI rules: Slint is never allowed; only the designated Tauri host may use Tauri.
    for package, path in HOST_PACKAGES.items():
        host_ui = dependencies(
            path, ("dependencies", "dev-dependencies", "build-dependencies"),
        ) & UI_DEPENDENCIES
        if "slint" in host_ui:
            fail(f"host package {package} has forbidden UI dependency ['slint']")
        if "tauri" in host_ui and package != TAURI_HOST:
            fail(f"only {TAURI_HOST} may depend on tauri; {package} does too")

    for package, deps in graph.items():
        if package != "graphene-network" and "reqwest" in deps:
            fail(f"reqwest escaped network boundary into {package}")

    # Engine packages must never depend on a host package.
    for package, deps in graph.items():
        leaked = deps & HOST_INTERNAL
        if leaked:
            fail(f"engine package {package} depends on host package {sorted(leaked)}")

    # Host packages use the root facade, never internal engine crates, and never reqwest.
    for package, deps in host_graph.items():
        if "reqwest" in deps:
            fail(f"host package {package} must not depend on reqwest directly")
        if "graphene" not in deps:
            fail(f"host package {package} must depend on the root `graphene` facade")
        for dependency in sorted(deps):
            if dependency in HOST_INTERNAL or dependency == "graphene":
                continue
            if dependency in ENGINE_INTERNAL:
                fail(
                    f"host package {package} reaches around the facade into {dependency}",
                )

    for source, destination in EXPLICIT_FORBIDDEN:
        if destination in graph[source]:
            fail(f"forbidden edge {source} -> {destination}")

    unexpected_core = graph["graphene-core"] - CORE_ALLOWED
    if unexpected_core:
        fail(f"graphene-core has infrastructure dependency {sorted(unexpected_core)}")

    # The combined workspace graph must remain acyclic. Hosts may depend on the root facade; engine
    # packages never depend on hosts, so this also preserves the engine-only invariant.
    combined_graph = {**graph, **host_graph}
    combined_internal = ENGINE_INTERNAL | HOST_INTERNAL

    def internal_edges(package: str) -> set[str]:
        return {dependency for dependency in combined_graph[package] if
            dependency in combined_internal}

    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(package: str) -> None:
        if package in visited:
            return
        if package in visiting:
            fail(f"dependency cycle includes {package}")
        visiting.add(package)
        for dependency in internal_edges(package):
            visit(dependency)
        visiting.remove(package)
        visited.add(package)

    for package in combined_graph:
        visit(package)

    for package in ENGINE_PACKAGES:
        for source_path in rust_sources(package):
            text = source_path.read_text(encoding="utf-8")
            relative = source_path.relative_to(ROOT)
            if package != "graphene-network" and re.search(r"\breqwest\s*::", text):
                fail(f"reqwest implementation type referenced by {relative}")
            if package not in {"graphene-providers", "graphene-modpack"} and DTO_TYPE_RE.search(
                    text,
            ):
                fail(f"provider DTO-like type leaked into {relative}")
            if package == "graphene-providers" and PUBLIC_DTO_RE.search(text):
                fail(f"provider DTO publicly exported from {relative}")
            if package == "graphene-service" and re.search(
                    r"graphene_providers::.*(?:fabric|forge|neoforge).*dto", text, re.DOTALL,
            ):
                fail(f"service imports a private loader DTO module in {relative}")
            if package in {"graphene-minecraft", "graphene-install"} and re.search(
                    r"\b(?:sh|bash|cmd\.exe|powershell)\b.*(?:-c|/C|-Command)", text, re.IGNORECASE,
            ):
                fail(f"domain/install source contains a shell execution pattern in {relative}")

    for package in HOST_PACKAGES:
        for source_path in rust_sources(package):
            text = source_path.read_text(encoding="utf-8")
            relative = source_path.relative_to(ROOT)
            if re.search(r"\breqwest\s*::", text):
                fail(f"host source references reqwest directly in {relative}")
            if ENGINE_NAMESPACE_RE.search(text):
                fail(f"host source reaches around the facade into an engine crate in {relative}")
            if re.search(
                    r"\b(?:sh|bash|cmd\.exe|powershell)\b.*(?:-c|/C|-Command)", text, re.IGNORECASE,
            ):
                fail(f"host source contains a shell execution pattern in {relative}")

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
            f"duplicate implementations found: {duplicated}",
        )

    for package in ENGINE_PACKAGES:
        facade = crate_root(package)
        if not facade.is_file():
            fail(f"crate root is missing for {package}")
        facade_text = facade.read_text(encoding="utf-8")
        line_count = len(facade_text.splitlines())
        if line_count > MAX_CRATE_ROOT_LINES:
            fail(
                f"{package} crate root has {line_count} lines; "
                f"crate roots must remain thin facades (max {MAX_CRATE_ROOT_LINES})",
            )
        if BUSINESS_ITEM_RE.search(facade_text):
            fail(f"{package} crate root defines business items instead of acting as a facade")

    for package in HOST_PACKAGES:
        base = package_dir(package)
        facade = base / "src" / "lib.rs"
        if not facade.is_file():
            fail(f"host crate root is missing for {package}")
        facade_text = facade.read_text(encoding="utf-8")
        line_count = len(facade_text.splitlines())
        if line_count > MAX_CRATE_ROOT_LINES:
            fail(
                f"host {package} lib.rs has {line_count} lines; "
                f"host entry points must remain thin (max {MAX_CRATE_ROOT_LINES})",
            )
        if BUSINESS_ITEM_RE.search(facade_text):
            fail(f"host {package} lib.rs defines business items instead of acting as a facade")
        main = base / "src" / "main.rs"
        if main.is_file():
            main_lines = len(main.read_text(encoding="utf-8").splitlines())
            if main_lines > MAX_HOST_MAIN_LINES:
                fail(
                    f"host {package} main.rs has {main_lines} lines; "
                    f"host executables must stay thin (max {MAX_HOST_MAIN_LINES})",
                )

    root_members = workspace_members(ROOT / "Cargo.toml")
    app_member = [member for member in root_members if member.startswith("apps")]
    if app_member:
        fail(f"engine workspace must not include host packages: {app_member}")

    app_members = {
        member.replace("\\", "/") for member in workspace_members(ROOT / "apps" / "Cargo.toml")
    }
    for package in HOST_PACKAGES:
        relative = package_dir(package).relative_to(ROOT / "apps").as_posix()
        if relative not in app_members:
            fail(f"host package {package} is missing from the apps workspace members")

    print("Graphene architecture manifest/source checks passed.")


if __name__ == "__main__":
    main()
