#!/usr/bin/env python3
"""Owner-map path validator (GOV-004).

Validates the `Main Source File` column of Markdown owner tables (GLOSSARY.md
and fixtures shaped like it): every owner link must be a portable,
repository-relative path that resolves to a real file or directory inside the
repository.

Strictly a PATH tool: it never inspects Rust symbols and never infers runtime
behavior.

Failure classes (severity `error` unless noted):
- MALFORMED_LINK           cell has no parseable Markdown link destination
- NON_PATH_LINK            http:/https:/mailto:/other scheme in the owner cell
- LEGACY_FILE_URI          machine-specific file:///.../<repo>/... link;
                           migration-required with the exact replacement
- INVALID_LEGACY_FILE_URI  file:// link that does not resolve inside the repo
- TEMPORARY_ABSOLUTE_PATH  /tmp, /var/folders, /private/tmp fiction
- ABSOLUTE_PATH            any other absolute path (not portable)
- GLOB_PATH                glob syntax (*, ?, [..], **) in the destination
- REPO_ESCAPE              path (or its symlink resolution) escapes the repo
- MISSING_PATH             portable path that does not exist; suggests up to
                           three close repository paths via difflib
- HISTORICAL (info)        missing portable path explicitly allowed by the
                           `<!-- owner-map:historical -->` marker in the SAME
                           table cell. The marker never waives /tmp, glob,
                           traversal, absolute, or malformed-link failures.

Exit codes:
  0  no errors and no migration-required legacy links
  1  validation or migration-required findings
  2  usage or internal parser error
"""

from __future__ import annotations

import argparse
import difflib
import json
import os
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path
from urllib.parse import unquote, urlparse

HISTORICAL_MARKER = "<!-- owner-map:historical -->"
SOURCE_HEADER = "Main Source File"
TEMPORARY_PREFIXES = ("/tmp", "/private/tmp", "/var/folders", "/private/var/folders")
GLOB_CHARS = ("*", "?", "[", "]")
CANDIDATE_EXCLUDED_DIRS = {
    ".git",
    "target",
    "target-agent",
    "node_modules",
    ".artifacts",
    ".notes",
    "vendor",
}


@dataclass(frozen=True)
class Finding:
    severity: str  # "error" | "migration-required" | "info"
    code: str
    markdown_file: str
    line: int
    row: str
    label: str
    target: str
    normalized_path: str | None
    suggestions: tuple[str, ...] = ()
    replacement: str | None = None
    message: str = ""


@dataclass
class Report:
    markdown_files: list[str] = field(default_factory=list)
    checked_rows: int = 0
    checked_links: int = 0
    resolved_paths: int = 0
    historical_allowed: int = 0
    findings: list[Finding] = field(default_factory=list)

    @property
    def errors(self) -> list[Finding]:
        return [f for f in self.findings if f.severity == "error"]

    @property
    def migration_required(self) -> list[Finding]:
        return [f for f in self.findings if f.severity == "migration-required"]

    def count(self, code: str) -> int:
        return sum(1 for f in self.findings if f.code == code)

    def to_json(self) -> dict:
        return {
            "schemaVersion": 1,
            "taskId": "GOV-004",
            "tool": "scripts/validate-owner-map-paths.py",
            "markdownFiles": self.markdown_files,
            "checkedRows": self.checked_rows,
            "checkedLinks": self.checked_links,
            "resolvedPaths": self.resolved_paths,
            "historicalAllowed": self.historical_allowed,
            "legacyFileUris": self.count("LEGACY_FILE_URI"),
            "absoluteTemporaryPaths": self.count("TEMPORARY_ABSOLUTE_PATH"),
            "globs": self.count("GLOB_PATH"),
            "missingPaths": self.count("MISSING_PATH"),
            "escapedRepoPaths": self.count("REPO_ESCAPE"),
            "errors": [asdict(f) for f in self.errors],
            "migrationRequired": [asdict(f) for f in self.migration_required],
            "infos": [asdict(f) for f in self.findings if f.severity == "info"],
            "pass": not self.errors and not self.migration_required,
        }


def split_markdown_table_row(line: str) -> list[str]:
    """Split one Markdown table row into trimmed cells.

    Small state machine instead of a naive `split("|")`: pipes inside
    inline code spans (`...`) and escaped pipes (\\|) stay inside their cell.
    Link destinations `[label](dest)` cannot contain a raw `|` in this
    corpus, but parentheses inside cells never confuse the cell splitter.
    """
    text = line.strip()
    if text.startswith("|"):
        text = text[1:]
    if text.endswith("|"):
        text = text[:-1]
    cells: list[str] = []
    current: list[str] = []
    in_code = False
    index = 0
    while index < len(text):
        char = text[index]
        if char == "\\" and index + 1 < len(text):
            current.append(text[index : index + 2])
            index += 2
            continue
        if char == "`":
            in_code = not in_code
            current.append(char)
        elif char == "|" and not in_code:
            cells.append("".join(current).strip())
            current = []
        else:
            current.append(char)
        index += 1
    cells.append("".join(current).strip())
    return cells


def is_separator_row(cells: list[str]) -> bool:
    return all(
        cell and set(cell) <= set(":- ") for cell in cells if cell
    ) and any("-" in cell for cell in cells)


def normalize_row_name(cell: str) -> str:
    return cell.replace("**", "").replace("`", "").strip()


def owner_table_rows(lines: list[str]):
    """Yield (line_no, row_label, source_cell) for every owner-table data row.

    A table is an owner table when its header row contains the exact
    `Main Source File` column header. Column position is tracked per table;
    leaving the table (non-`|` line) resets it.
    """
    active_source_column: int | None = None
    for line_no, line in enumerate(lines, start=1):
        if not line.lstrip().startswith("|"):
            active_source_column = None
            continue
        cells = split_markdown_table_row(line)
        if SOURCE_HEADER in cells:
            active_source_column = cells.index(SOURCE_HEADER)
            continue
        if active_source_column is None or is_separator_row(cells):
            continue
        if active_source_column >= len(cells):
            continue
        yield line_no, normalize_row_name(cells[0]), cells[active_source_column]


def parse_link_destinations(cell: str) -> list[tuple[str, str]]:
    """Extract every `[label](destination)` pair from one table cell.

    State machine (not regex): labels may contain brackets in code spans;
    destinations may contain balanced parentheses.
    """
    links: list[tuple[str, str]] = []
    index = 0
    while index < len(cell):
        if cell[index] != "[":
            index += 1
            continue
        depth = 1
        label_start = index + 1
        cursor = label_start
        while cursor < len(cell) and depth > 0:
            if cell[cursor] == "[":
                depth += 1
            elif cell[cursor] == "]":
                depth -= 1
            cursor += 1
        if depth != 0 or cursor >= len(cell) or cell[cursor] != "(":
            index += 1
            continue
        label = cell[label_start : cursor - 1]
        dest_start = cursor + 1
        depth = 1
        cursor = dest_start
        while cursor < len(cell) and depth > 0:
            if cell[cursor] == "(":
                depth += 1
            elif cell[cursor] == ")":
                depth -= 1
            cursor += 1
        if depth != 0:
            index += 1
            continue
        links.append((label.strip(), cell[dest_start : cursor - 1].strip()))
        index = cursor
    return links


def split_fragment(target: str) -> tuple[str, str]:
    """Strip a URL fragment; preserved for Markdown, ignored for resolution."""
    if "#" in target:
        base, fragment = target.split("#", 1)
        return base, fragment
    return target, ""


def legacy_repo_relative_path(absolute: Path, repo_root: Path) -> Path | None:
    """Map a legacy absolute path from ANY machine onto this repo.

    Accepts both a path inside the current repo root and a foreign
    `/Users/<someone>/.../<repo-dir-name>/<relative>` spelling.
    """
    try:
        return absolute.relative_to(repo_root)
    except ValueError:
        pass
    parts = absolute.parts
    repo_name = repo_root.name
    for index, part in enumerate(parts):
        if part == repo_name and index + 1 <= len(parts):
            candidate = Path(*parts[index + 1 :]) if index + 1 < len(parts) else Path(".")
            return candidate
    return None


def normalize_link_target(raw: str, repo_root: Path):
    """Classify one link destination.

    Returns (kind, path_or_none, replacement_or_none, fragment) where kind is
    one of: "legacy", "invalid-legacy", "non-path", "relative".
    """
    stripped = raw.strip().strip("<>")
    base, fragment = split_fragment(stripped)
    parsed = urlparse(base)
    if parsed.scheme == "file":
        absolute = Path(unquote(parsed.path))
        relative = legacy_repo_relative_path(absolute, repo_root)
        if relative is None:
            return "invalid-legacy", None, None, fragment
        replacement = relative.as_posix()
        if fragment:
            replacement += f"#{fragment}"
        return "legacy", relative, replacement, fragment
    if parsed.scheme:
        return "non-path", None, None, fragment
    return "relative", Path(unquote(base)), None, fragment


def is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False


def repository_candidates(repo_root: Path) -> list[str]:
    """Repository-relative file paths for near-match suggestions.

    Build output, dependency, artifact, and VCS directories are excluded so a
    suggestion never points at generated or vendored fiction.
    """
    candidates: list[str] = []
    for dirpath, dirnames, filenames in os.walk(repo_root):
        dirnames[:] = [
            name
            for name in dirnames
            if name not in CANDIDATE_EXCLUDED_DIRS and not name.startswith(".")
        ]
        relative_dir = Path(dirpath).relative_to(repo_root)
        for filename in filenames:
            relative = (relative_dir / filename).as_posix()
            if relative.startswith("./"):
                relative = relative[2:]
            candidates.append(relative)
    return candidates


def closest_paths(missing: Path, candidates: list[str]) -> tuple[str, ...]:
    target = missing.as_posix()
    basename_matches = [
        candidate for candidate in candidates if Path(candidate).name == missing.name
    ]
    fuzzy = difflib.get_close_matches(target, candidates, n=3, cutoff=0.45)
    ordered = list(dict.fromkeys(basename_matches + fuzzy))
    return tuple(ordered[:3])


def validate_owner_map(markdown_path: Path, repo_root: Path, report: Report) -> None:
    lines = markdown_path.read_text(encoding="utf-8").splitlines()
    report.markdown_files.append(str(markdown_path))
    resolved_root = repo_root.resolve()
    candidates: list[str] | None = None
    display = _display_path(markdown_path, repo_root)

    for line_no, row_label, source_cell in owner_table_rows(lines):
        report.checked_rows += 1
        historical = HISTORICAL_MARKER in source_cell
        links = parse_link_destinations(source_cell)
        if not links:
            report.findings.append(
                Finding(
                    severity="error",
                    code="MALFORMED_LINK",
                    markdown_file=display,
                    line=line_no,
                    row=f"row '{row_label}'",
                    label=row_label,
                    target=source_cell,
                    normalized_path=None,
                    message=(
                        f"row '{row_label}' has no parseable Markdown link in its "
                        f"'{SOURCE_HEADER}' cell"
                    ),
                )
            )
            continue

        for label, destination in links:
            report.checked_links += 1
            kind, path, replacement, _fragment = normalize_link_target(
                destination, repo_root
            )

            if kind == "invalid-legacy":
                report.findings.append(
                    Finding(
                        severity="error",
                        code="INVALID_LEGACY_FILE_URI",
                        markdown_file=display,
                        line=line_no,
                        row=f"row '{row_label}'",
                        label=label,
                        target=destination,
                        normalized_path=None,
                        message=(
                            f"row '{row_label}': file:// link does not resolve inside "
                            f"the repository"
                        ),
                    )
                )
                continue

            if kind == "legacy":
                # Migration-required regardless of whether the absolute path
                # happens to exist on THIS machine.
                report.findings.append(
                    Finding(
                        severity="migration-required",
                        code="LEGACY_FILE_URI",
                        markdown_file=display,
                        line=line_no,
                        row=f"row '{row_label}'",
                        label=label,
                        target=destination,
                        normalized_path=path.as_posix() if path else None,
                        replacement=replacement,
                        message=(
                            f"row '{row_label}': machine-specific file:// link; "
                            f"replace destination with '{replacement}'"
                        ),
                    )
                )
                continue

            if kind == "non-path":
                report.findings.append(
                    Finding(
                        severity="error",
                        code="NON_PATH_LINK",
                        markdown_file=display,
                        line=line_no,
                        row=f"row '{row_label}'",
                        label=label,
                        target=destination,
                        normalized_path=None,
                        message=(
                            f"row '{row_label}': '{SOURCE_HEADER}' must be a repository "
                            f"path, not a {urlparse(destination).scheme}: URL"
                        ),
                    )
                )
                continue

            assert path is not None
            posix = path.as_posix()

            # Unsafe path classes fail even under the historical marker.
            if posix.startswith(TEMPORARY_PREFIXES):
                report.findings.append(
                    Finding(
                        severity="error",
                        code="TEMPORARY_ABSOLUTE_PATH",
                        markdown_file=display,
                        line=line_no,
                        row=f"row '{row_label}'",
                        label=label,
                        target=destination,
                        normalized_path=posix,
                        message=(
                            f"row '{row_label}': temporary absolute path is fiction; "
                            f"owner paths must be repository-relative"
                        ),
                    )
                )
                continue
            if any(char in posix for char in GLOB_CHARS):
                report.findings.append(
                    Finding(
                        severity="error",
                        code="GLOB_PATH",
                        markdown_file=display,
                        line=line_no,
                        row=f"row '{row_label}'",
                        label=label,
                        target=destination,
                        normalized_path=posix,
                        message=(
                            f"row '{row_label}': glob syntax is not a resolvable owner "
                            f"path"
                        ),
                    )
                )
                continue
            if path.is_absolute():
                report.findings.append(
                    Finding(
                        severity="error",
                        code="ABSOLUTE_PATH",
                        markdown_file=display,
                        line=line_no,
                        row=f"row '{row_label}'",
                        label=label,
                        target=destination,
                        normalized_path=posix,
                        message=(
                            f"row '{row_label}': absolute path is not portable; use a "
                            f"repository-relative path"
                        ),
                    )
                )
                continue

            joined = (repo_root / path).resolve()
            if not is_within(joined, resolved_root):
                report.findings.append(
                    Finding(
                        severity="error",
                        code="REPO_ESCAPE",
                        markdown_file=display,
                        line=line_no,
                        row=f"row '{row_label}'",
                        label=label,
                        target=destination,
                        normalized_path=posix,
                        message=(
                            f"row '{row_label}': path escapes the repository root"
                        ),
                    )
                )
                continue

            if joined.exists():
                # A symlink that resolves outside the repo is an escape even
                # though the link itself sits inside.
                real = joined.resolve()
                if not is_within(real, resolved_root):
                    report.findings.append(
                        Finding(
                            severity="error",
                            code="REPO_ESCAPE",
                            markdown_file=display,
                            line=line_no,
                            row=f"row '{row_label}'",
                            label=label,
                            target=destination,
                            normalized_path=posix,
                            message=(
                                f"row '{row_label}': symlink resolves outside the "
                                f"repository root"
                            ),
                        )
                    )
                    continue
                report.resolved_paths += 1
                continue

            # Missing portable path: allowed only with the historical marker.
            if historical:
                report.historical_allowed += 1
                report.findings.append(
                    Finding(
                        severity="info",
                        code="HISTORICAL",
                        markdown_file=display,
                        line=line_no,
                        row=f"row '{row_label}'",
                        label=label,
                        target=destination,
                        normalized_path=posix,
                        message=(
                            f"row '{row_label}': missing path allowed by explicit "
                            f"historical marker"
                        ),
                    )
                )
                continue

            if candidates is None:
                candidates = repository_candidates(repo_root)
            suggestions = closest_paths(path, candidates)
            suffix = (
                f"; closest: {', '.join(suggestions)}"
                if suggestions
                else "; no near match"
            )
            report.findings.append(
                Finding(
                    severity="error",
                    code="MISSING_PATH",
                    markdown_file=display,
                    line=line_no,
                    row=f"row '{row_label}'",
                    label=label,
                    target=destination,
                    normalized_path=posix,
                    suggestions=suggestions,
                    message=(
                        f"row '{row_label}': path '{posix}' does not exist{suffix}"
                    ),
                )
            )


def _display_path(path: Path, repo_root: Path) -> str:
    try:
        return path.resolve().relative_to(repo_root.resolve()).as_posix()
    except ValueError:
        return str(path)


def find_repo_root(start: Path) -> Path:
    cursor = start.resolve()
    for candidate in (cursor, *cursor.parents):
        if (candidate / ".git").exists():
            return candidate
    return start.resolve()


def print_human(report: Report) -> None:
    for finding in report.findings:
        stream = sys.stdout if finding.severity == "info" else sys.stderr
        print(
            f"[{finding.severity}] {finding.code} {finding.markdown_file}:"
            f"{finding.line} {finding.message}",
            file=stream,
        )
        if finding.replacement:
            print(f"    replacement: {finding.replacement}", file=stream)
        for suggestion in finding.suggestions:
            print(f"    suggestion: {suggestion}", file=stream)
    print(
        f"owner-map: {report.checked_rows} rows, {report.checked_links} links, "
        f"{report.resolved_paths} resolved, {report.historical_allowed} historical, "
        f"{len(report.errors)} errors, {len(report.migration_required)} "
        f"migration-required"
    )


# ── Self-test ─────────────────────────────────────────────────────────────


def self_test() -> int:
    """Parser and classification self-tests against in-memory fixtures."""
    import tempfile

    failures: list[str] = []

    def check(name: str, condition: bool) -> None:
        if not condition:
            failures.append(name)

    # Cell splitting: pipes in code spans stay inside cells.
    cells = split_markdown_table_row("| **A** | uses `a|b` | [x](y) |")
    check("code-span pipe", cells == ["**A**", "uses `a|b`", "[x](y)"])

    # Link parsing: multiple links, parenthesized destinations, anchors.
    links = parse_link_destinations(
        "[a.rs](src/a.rs#L10) & [b (two).rs](src/b%20x.rs)"
    )
    check("multi-link parse", len(links) == 2)
    check("anchor kept in raw dest", links[0][1] == "src/a.rs#L10")

    with tempfile.TemporaryDirectory() as tmp:
        repo = Path(tmp)
        (repo / ".git").mkdir()
        (repo / "src").mkdir()
        (repo / "src" / "real.rs").write_text("// fixture\n")
        (repo / "src" / "notes_browse.rs").write_text("// fixture\n")

        fixture = repo / "owner-map.md"
        fixture.write_text(
            "\n".join(
                [
                    "| UI Element | Description | Main Source File |",
                    "| :--- | :--- | :--- |",
                    "| **Good** | ok | [real.rs](src/real.rs) |",
                    "| **Anchored** | ok | [real.rs](src/real.rs#L5) |",
                    (
                        "| **Legacy** | migrate | "
                        f"[real.rs](file://{repo}/src/real.rs#L2) |"
                    ),
                    (
                        "| **Foreign Legacy** | migrate | "
                        "[real.rs](file:///Users/someone/dev/"
                        f"{repo.name}/src/real.rs) |"
                    ),
                    "| **Tmp** | reject | [f.rs](/tmp/f.rs) |",
                    "| **Glob** | reject | [src](src/**) |",
                    "| **Escape** | reject | [up](../outside.rs) |",
                    "| **Missing** | suggest | [notes_brows.rs](src/notes_brows.rs) |",
                    (
                        "| **Historical** | allowed | [old.rs](src/old.rs) "
                        "<!-- owner-map:historical --> |"
                    ),
                    (
                        "| **Historical Tmp** | still rejected | [f.rs](/tmp/f.rs) "
                        "<!-- owner-map:historical --> |"
                    ),
                    "| **Web** | reject | [docs](https://example.com/a.rs) |",
                    "| **Malformed** | reject | just prose, no link |",
                ]
            )
            + "\n"
        )

        report = Report()
        validate_owner_map(fixture, repo, report)

        by_row = {f.label if f.code == "MALFORMED_LINK" else f.row: f for f in report.findings}
        codes = sorted(f.code for f in report.findings)
        check(
            "expected codes",
            codes
            == sorted(
                [
                    "LEGACY_FILE_URI",
                    "LEGACY_FILE_URI",
                    "TEMPORARY_ABSOLUTE_PATH",
                    "GLOB_PATH",
                    "REPO_ESCAPE",
                    "MISSING_PATH",
                    "HISTORICAL",
                    "TEMPORARY_ABSOLUTE_PATH",
                    "NON_PATH_LINK",
                    "MALFORMED_LINK",
                ]
            ),
        )
        check("good rows resolved", report.resolved_paths == 2)
        legacy = [f for f in report.findings if f.code == "LEGACY_FILE_URI"]
        check(
            "legacy replacement exact",
            any(f.replacement == "src/real.rs#L2" for f in legacy),
        )
        check(
            "foreign legacy replacement exact",
            any(f.replacement == "src/real.rs" for f in legacy),
        )
        missing = [f for f in report.findings if f.code == "MISSING_PATH"]
        check("missing has row name", missing and "row 'Missing'" in missing[0].row)
        check(
            "missing suggests near match",
            missing and "src/notes_browse.rs" in missing[0].suggestions,
        )
        historical = [f for f in report.findings if f.code == "HISTORICAL"]
        check("historical allowed once", len(historical) == 1)
        check(
            "historical never waives tmp",
            sum(1 for f in report.findings if f.code == "TEMPORARY_ABSOLUTE_PATH") == 2,
        )
        check("report fails", report.to_json()["pass"] is False)

        # A clean fixture passes.
        clean = repo / "clean.md"
        clean.write_text(
            "\n".join(
                [
                    "| UI Element | Description | Main Source File |",
                    "| :--- | :--- | :--- |",
                    "| **Good** | ok | [real.rs](src/real.rs) |",
                ]
            )
            + "\n"
        )
        clean_report = Report()
        validate_owner_map(clean, repo, clean_report)
        check("clean passes", clean_report.to_json()["pass"] is True)

    if failures:
        for name in failures:
            print(f"[self-test] FAIL: {name}", file=sys.stderr)
        return 1
    print("[self-test] all owner-map validator self-tests passed")
    return 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description="Validate owner-map Main Source File paths (GOV-004)."
    )
    parser.add_argument("markdown", nargs="*", help="Markdown files to validate")
    parser.add_argument("--json-out", help="Write a JSON report to this path")
    parser.add_argument(
        "--repo-root", help="Repository root (default: discovered from cwd)"
    )
    parser.add_argument(
        "--self-test", action="store_true", help="Run parser/classification self-tests"
    )
    try:
        options = parser.parse_args(argv)
    except SystemExit as exit_error:
        return 2 if exit_error.code not in (0, None) else 0

    if options.self_test:
        return self_test()

    if not options.markdown:
        parser.print_usage(sys.stderr)
        print("error: at least one Markdown file is required", file=sys.stderr)
        return 2

    repo_root = (
        Path(options.repo_root).resolve()
        if options.repo_root
        else find_repo_root(Path.cwd())
    )

    report = Report()
    try:
        for markdown in options.markdown:
            markdown_path = Path(markdown)
            if not markdown_path.exists():
                print(f"error: no such file: {markdown}", file=sys.stderr)
                return 2
            validate_owner_map(markdown_path, repo_root, report)
    except Exception as error:  # parser/internal error is exit 2, never silent
        print(f"error: internal validator failure: {error}", file=sys.stderr)
        return 2

    if options.json_out:
        out_path = Path(options.json_out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        temporary = out_path.with_suffix(out_path.suffix + f".tmp-{os.getpid()}")
        temporary.write_text(json.dumps(report.to_json(), indent=2) + "\n")
        temporary.replace(out_path)

    print_human(report)
    return 0 if (not report.errors and not report.migration_required) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
