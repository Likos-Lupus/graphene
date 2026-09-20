#!/usr/bin/env python3
"""Stable repository hygiene checks for source size, test naming, CI, and active docs."""

from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
MAX_SOURCE_LINES = 1000
REVIEW_LINES = 500
SPLIT_EXPECTED_LINES = 800
LARGE_INLINE_TEST_LINES = 100
MAX_FRONTEND_LINES = 500
REVIEW_FRONTEND_LINES = 300
EXCLUDED_SOURCE_DIRS = {"node_modules", "dist", "target", "gen", ".vite"}

PHASE_TOKEN_RE = re.compile(r"(?i)\bphase[_ -]?[0-9]+\b")
PHASE_IDENTIFIER_RE = re.compile(r"(?i)phase_?[0-9]+")
PHASE_TEST_FN_RE = re.compile(
    r"(?ms)#\[(?:tokio::)?test(?:\([^\]]*\))?\].*?\bfn\s+([A-Za-z_][A-Za-z0-9_]*)",
)
PHASE_TEST_MOD_RE = re.compile(
    r"(?m)#\[cfg\(test\)\]\s*(?:#\[[^\]]+\]\s*)*mod\s+([A-Za-z_][A-Za-z0-9_]*)",
)
ACTIVE_PHASE_DOC_RE = re.compile(
    r"^PHASE_[0-9]+_(?:IMPLEMENTATION_PLAN|API|SECURITY_REVIEW|.*SMOKE_TEST|SUPPORT_MATRIX)\.md$",
)


def fail(messages: list[str]) -> None:
    if messages:
        formatted = "\n".join(f"- {message}" for message in messages)
        raise SystemExit(f"hygiene check failed:\n{formatted}")


def _excluded(path: pathlib.Path, base: pathlib.Path) -> bool:
    return bool(EXCLUDED_SOURCE_DIRS.intersection(path.relative_to(base).parts))


def rust_sources() -> list[pathlib.Path]:
    roots = [ROOT / "src", ROOT / "crates", ROOT / "apps"]
    sources: list[pathlib.Path] = []
    for source_root in roots:
        if not source_root.exists():
            continue
        for path in source_root.rglob("*.rs"):
            if _excluded(path, source_root):
                continue
            sources.append(path)
    return sorted(sources)


def frontend_sources() -> list[pathlib.Path]:
    root = ROOT / "apps"
    if not root.exists():
        return []
    return sorted(
        path for path in root.rglob("*.ts") if not _excluded(path, root)
    )


def is_test_source(path: pathlib.Path) -> bool:
    relative = path.relative_to(ROOT)
    return "tests" in relative.parts or path.name == "tests.rs"


def inline_test_start(lines: list[str]) -> int | None:
    for index, line in enumerate(lines[:-1]):
        if line.strip() != "#[cfg(test)]":
            continue
        following = lines[index + 1].strip()
        if re.match(r"mod\s+[A-Za-z_][A-Za-z0-9_]*\s*\{", following):
            return index
    return None


def production_line_count(path: pathlib.Path) -> int:
    lines = path.read_text(encoding="utf-8").splitlines()
    inline_start = inline_test_start(lines)
    return inline_start if inline_start is not None else len(lines)


def inline_test_lines(path: pathlib.Path) -> int:
    lines = path.read_text(encoding="utf-8").splitlines()
    inline_start = inline_test_start(lines)
    return 0 if inline_start is None else len(lines) - inline_start


def check_ci_phase_names(errors: list[str]) -> None:
    workflows = ROOT / ".github" / "workflows"
    if not workflows.exists():
        return
    for path in sorted(workflows.iterdir()):
        if not path.is_file():
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if re.match(r"^\s*name\s*:", line) and PHASE_TOKEN_RE.search(line):
                errors.append(
                    f"phase-numbered CI display name in {path.relative_to(ROOT)}:{number}",
                )


def check_phase_tests(path: pathlib.Path, errors: list[str]) -> None:
    text = path.read_text(encoding="utf-8")
    for match in PHASE_TEST_FN_RE.finditer(text):
        if PHASE_IDENTIFIER_RE.search(match.group(1)):
            errors.append(
                f"phase-numbered test function {match.group(1)} in {path.relative_to(ROOT)}",
            )
    for match in PHASE_TEST_MOD_RE.finditer(text):
        if PHASE_IDENTIFIER_RE.search(match.group(1)):
            errors.append(
                f"phase-numbered test module {match.group(1)} in {path.relative_to(ROOT)}",
            )


def main() -> None:
    errors: list[str] = []
    review: list[tuple[int, pathlib.Path]] = []
    split_expected: list[tuple[int, pathlib.Path]] = []
    large_inline_tests: list[tuple[int, pathlib.Path]] = []

    for path in rust_sources():
        check_phase_tests(path, errors)
        if is_test_source(path):
            continue
        lines = production_line_count(path)
        relative = path.relative_to(ROOT)
        if lines > MAX_SOURCE_LINES:
            errors.append(f"hand-written Rust source has {lines} production lines: {relative}")
        elif lines >= SPLIT_EXPECTED_LINES:
            split_expected.append((lines, relative))
        elif lines >= REVIEW_LINES:
            review.append((lines, relative))

        test_lines = inline_test_lines(path)
        if test_lines >= LARGE_INLINE_TEST_LINES:
            large_inline_tests.append((test_lines, relative))

    for path in frontend_sources():
        lines = len(path.read_text(encoding="utf-8").splitlines())
        relative = path.relative_to(ROOT)
        if lines > MAX_FRONTEND_LINES:
            errors.append(f"frontend TypeScript has {lines} lines: {relative}")
        elif lines >= REVIEW_FRONTEND_LINES:
            review.append((lines, relative))

    check_ci_phase_names(errors)

    docs = ROOT / "docs"
    if docs.exists():
        for path in docs.glob("PHASE_*.md"):
            if ACTIVE_PHASE_DOC_RE.match(path.name):
                errors.append(f"active phase-owned document remains: {path.relative_to(ROOT)}")

    fail(errors)

    for label, values in (
            ("review 500-799", review),
            ("split expected 800-999", split_expected),
            ("large inline tests", large_inline_tests),
    ):
        if values:
            print(f"{label}:")
            for lines, path in sorted(values, reverse=True):
                print(f"  {lines:4}  {path}")

    markdown = sorted(docs.rglob("*.md")) if docs.exists() else []
    markdown_lines = sum(len(path.read_text(encoding="utf-8").splitlines()) for path in markdown)
    print(f"documentation: {len(markdown)} markdown files, {markdown_lines} lines")
    print("Graphene hygiene checks passed.")


if __name__ == "__main__":
    main()
