"""Verify that cited revisions and evidence artifacts still resolve.

The board and the verification documents name the revision each increment landed
at and the SHA-256 of the artifact it produced. Two kinds of rot are silent:

* A revision named after the fact is often the branch commit that was rewritten
  before landing, so no reader can resolve it from the default branch. Only
  commits that are objects in this repository are considered, so a revision
  belonging to another repository is left alone.
* An artifact under the git-ignored `output/` tree is host state, so a document
  can outlive the only copy of the evidence it pins.

`check` fails on an unreachable revision and on a digest that contradicts an
artifact that is present. An absent artifact is reported without failing: that
is the expected state on a host which never ran the capture, and the exit status
must not depend on which probes a host happened to run.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import pathlib
import re
import subprocess
import sys
from collections.abc import Callable, Iterator

ROOT = pathlib.Path(__file__).resolve().parents[1]
DOCUMENTS = ("backlog.md", "README.md", "docs")
SKIPPED_DIRECTORIES = frozenset({".git", "target", "node_modules", "__pycache__", "output", "dist"})
REVISION = re.compile(r"(?<![0-9a-fA-F])(?P<sha>[0-9a-f]{7,40})(?![0-9a-fA-F])")
# A digest citation may wrap across lines, so the document is matched with its
# whitespace collapsed. The digest must also be introduced close to the artifact
# it belongs to: every delivered citation uses a short connector ("` with
# SHA-256", "` (SHA-256", "` fixture, whose committed SHA-256"). A wider gap
# means the sentence goes on to describe a different artifact -- a trace's stored
# screenshot, for example -- and binding that digest to the trace file would
# report a mismatch for a citation that is in fact correct.
ARTIFACT = r"[A-Za-z0-9_./-]+\.(?:json|wav|woff2|png|jpg|jpeg|html|log|txt|xml)"
DIGEST = re.compile(
    rf"(?P<artifact>{ARTIFACT})[\s\S]{{0,32}}?SHA-256[\s\S]{{0,16}}?(?P<digest>[0-9a-f]{{64}})",
    re.IGNORECASE,
)
WHITESPACE = re.compile(r"\s+")


def documents(root: pathlib.Path) -> Iterator[pathlib.Path]:
    """Yield every Markdown document that can cite a revision or an artifact."""
    for name in DOCUMENTS:
        target = root / name
        if target.is_file():
            yield target
        elif target.is_dir():
            for base, directories, files in os.walk(target):
                directories[:] = [name for name in directories if name not in SKIPPED_DIRECTORIES]
                yield from (pathlib.Path(base) / file for file in sorted(files) if file.endswith(".md"))


def _git(root: pathlib.Path, *arguments: str) -> tuple[int, str]:
    result = subprocess.run(
        ["git", *arguments],
        cwd=root,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    return result.returncode, result.stdout.strip()


def _resolver(root: pathlib.Path, reference: str) -> Callable[[str], bool | None]:
    """Return a probe reporting whether one cited commit is reachable."""

    def resolve(sha: str) -> bool | None:
        code, kind = _git(root, "cat-file", "-t", sha)
        if code != 0 or kind != "commit":
            # Another repository's revision, an image or a hosted run identifier.
            return None
        return _git(root, "merge-base", "--is-ancestor", sha, reference)[0] == 0

    return resolve


def unreachable_revisions(
    root: pathlib.Path,
    reference: str = "HEAD",
    *,
    resolve: Callable[[str], bool | None] | None = None,
) -> list[str]:
    """Return citations of this repository's commits that `reference` cannot reach."""
    resolve = resolve or _resolver(root, reference)
    findings: list[str] = []
    known: dict[str, bool | None] = {}
    for document in documents(root):
        text = document.read_text(encoding="utf-8", errors="replace")
        for number, line in enumerate(text.splitlines(), 1):
            if "SHA-256" in line:
                continue
            for match in REVISION.finditer(line):
                sha = match.group("sha")
                if sha not in known:
                    known[sha] = resolve(sha)
                if known[sha] is False:
                    _, subject = _git(root, "log", "-1", "--format=%s", sha)
                    findings.append(
                        f"{document.relative_to(root).as_posix()}:{number}: "
                        f"{sha[:12]} is not reachable from {reference} ({subject[:60]})"
                    )
    return findings


def evidence_findings(root: pathlib.Path) -> tuple[list[str], list[str], int]:
    """Return contradicted digests, absent artifacts and the count verified."""
    mismatched: list[str] = []
    absent: list[str] = []
    verified = 0
    for document in documents(root):
        raw = document.read_text(encoding="utf-8", errors="replace")
        for match in DIGEST.finditer(WHITESPACE.sub(" ", raw)):
            artifact, expected = match.group("artifact"), match.group("digest").lower()
            line = raw[: raw.find(artifact)].count("\n") + 1
            where = f"{document.relative_to(root).as_posix()}:{line}: {artifact}"
            candidate = root / artifact
            if not candidate.is_file():
                absent.append(f"{where}: cited artifact is absent on this host")
                continue
            actual = hashlib.sha256(candidate.read_bytes()).hexdigest()
            if actual == expected:
                verified += 1
            else:
                mismatched.append(f"{where}: cited {expected[:12]} but the artifact is {actual[:12]}")
    return mismatched, absent, verified


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=pathlib.Path, default=ROOT)
    parser.add_argument("--reference", default="HEAD",
                        help="revision every cited commit must be reachable from")
    parser.add_argument("--require-artifacts", action="store_true",
                        help="also fail when a cited artifact is absent from this host")
    arguments = parser.parse_args()

    root = arguments.root.resolve()
    unreachable = unreachable_revisions(root, arguments.reference)
    mismatched, absent, verified = evidence_findings(root)

    for finding in unreachable:
        print(f"unreachable revision: {finding}", file=sys.stderr)
    for finding in mismatched:
        print(f"evidence mismatch: {finding}", file=sys.stderr)
    for finding in absent:
        print(f"evidence absent: {finding}", file=sys.stderr)
    print(
        f"citations: {verified} digest(s) verified, {len(unreachable)} unreachable revision(s), "
        f"{len(mismatched)} mismatch(es), {len(absent)} absent artifact(s)",
        flush=True,
    )
    if unreachable or mismatched or (arguments.require_artifacts and absent):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
