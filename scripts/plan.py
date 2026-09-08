"""Validate the Metis plan identifiers, dependencies and local links."""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
from urllib.parse import unquote, urlsplit

ROOT = pathlib.Path(__file__).resolve().parents[1]
ITEM = re.compile(r"^##\s+(?P<id>[A-Z][A-Z0-9-]*)\s+—\s+")
ANCHOR = re.compile(r'^<a id="(?P<id>[A-Za-z0-9_-]+)"></a>$')
DEPENDENCIES = re.compile(r"^- Dependencies:\s*(?P<value>[^\r\n]*)$")
STATUS = re.compile(r"^- Status:\s*(?P<value>[a-z-]+)")
LINK = re.compile(r"\]\((?P<destination>[^)\r\n]+)\)")
ID = re.compile(r"\A[A-Z][A-Z0-9-]*\Z")
STATUSES = frozenset({"todo", "in-progress", "blocked", "review", "done"})


def _local_target(document: pathlib.Path, destination: str, root: pathlib.Path) -> tuple[pathlib.Path, str] | None:
    """Return a local Markdown target, leaving remote and stack links alone."""
    value = destination.strip().split(maxsplit=1)[0].strip("<>")
    parsed = urlsplit(value)
    if parsed.scheme or parsed.netloc:
        return None
    path = (document.parent / unquote(parsed.path)).resolve() if parsed.path else document
    if not path.is_relative_to(root):
        # The member board intentionally links upward to the Atlas and ritk
        # boards; those files do not exist in a standalone Metis checkout.
        return None
    return path, parsed.fragment


def _anchors(path: pathlib.Path) -> set[str]:
    return {
        match.group("id")
        for line in path.read_text(encoding="utf-8").splitlines()
        if (match := ANCHOR.fullmatch(line.strip()))
    }


def _fragment_names(path: pathlib.Path) -> set[str]:
    """Return explicit anchors and the GitHub-style slugs for headings."""
    names = _anchors(path)
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("#"):
            continue
        raw = line.lstrip("#").strip().casefold()
        removed = re.sub(r"[^\w\s-]", "", raw)
        names.add(re.sub(r"\s+", "-", removed).strip("-"))
        replaced = re.sub(r"[^\w\s-]", "-", raw)
        slug = re.sub(r"\s+", "-", replaced).strip("-")
        names.add(re.sub(r"-{3,}", "--", slug))
    return names


def check_plan(root: pathlib.Path = ROOT) -> list[str]:
    """Return every plan-integrity finding in *root* in deterministic order."""
    root = root.resolve()
    findings: list[str] = []
    backlog = root / "backlog.md"
    checklist = root / "checklist.md"
    if not backlog.is_file():
        return [f"missing plan board: {backlog}"]

    lines = backlog.read_text(encoding="utf-8").splitlines()
    item_ids: list[str] = []
    item_bodies: list[tuple[str, list[str], int]] = []
    current: tuple[str, list[str], int] | None = None
    for number, line in enumerate(lines, 1):
        match = ITEM.match(line)
        if match:
            if current is not None:
                item_bodies.append(current)
            item_id = match.group("id")
            item_ids.append(item_id)
            if number == 1 or lines[number - 2].strip() != f'<a id="{item_id}"></a>':
                findings.append(f"{backlog}:{number}: item lacks its anchor: {item_id}")
            current = (item_id, [], number)
        elif current is not None:
            current[1].append(line)
    if current is not None:
        item_bodies.append(current)

    duplicates = sorted({item_id for item_id in item_ids if item_ids.count(item_id) > 1})
    findings.extend(f"{backlog}: duplicate item id: {item_id}" for item_id in duplicates)
    defined = set(item_ids)
    board_anchors = _anchors(backlog)
    for anchor in sorted(board_anchors - defined):
        findings.append(f"{backlog}: anchor has no item: {anchor}")
    for item_id in sorted(defined - board_anchors):
        findings.append(f"{backlog}: item has no anchor: {item_id}")

    for item_id, body, line_number in item_bodies:
        status_lines = [STATUS.match(line) for line in body]
        statuses = [match.group("value") for match in status_lines if match]
        if len(statuses) != 1:
            findings.append(f"{backlog}:{line_number}: item must have one Status: {item_id}")
        elif statuses[0] not in STATUSES:
            findings.append(f"{backlog}:{line_number}: unknown status {statuses[0]}: {item_id}")
        for dependency in (match.group("value").split(";", 1)[0].strip()
                           for line in body if (match := DEPENDENCIES.match(line))):
            if not dependency:
                continue
            for dependency_id in (value.strip().rstrip(".") for value in dependency.split(",")):
                if not ID.fullmatch(dependency_id) or dependency_id not in defined:
                    findings.append(
                        f"{backlog}:{line_number}: unknown dependency {dependency_id}: {item_id}"
                    )

    for document in (backlog, checklist):
        if not document.is_file():
            findings.append(f"missing plan document: {document}")
            continue
        for number, line in enumerate(document.read_text(encoding="utf-8").splitlines(), 1):
            for match in LINK.finditer(line):
                target = _local_target(document, match.group("destination"), root)
                if target is None:
                    continue
                path, fragment = target
                if not path.is_file():
                    findings.append(f"{document}:{number}: missing local link target: {path}")
                elif fragment and fragment not in _fragment_names(path):
                    findings.append(f"{document}:{number}: missing local link anchor: {fragment}")
    return sorted(findings)


def main() -> int:
    """Run the plan integrity check and report actionable findings."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("check",))
    args = parser.parse_args()
    del args
    findings = check_plan()
    if findings:
        print("\n".join(findings), file=sys.stderr)
        return 1
    print("Plan checks passed: item anchors, statuses, dependencies and local links")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
