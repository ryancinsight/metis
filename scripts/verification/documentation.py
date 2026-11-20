"""Verification of local documentation links and evidence citations."""

import re
import sys
from urllib.parse import unquote, urlsplit


def manual_links(root, physical_root):
    """Check inline Markdown file and image destinations without network requests."""
    for document in sorted((root / "docs" / "manual").rglob("*.md")):
        for match in re.finditer(
            r"!?\[[^\]\n]*\]\(([^)\n]+)\)", document.read_text(encoding="utf-8")
        ):
            destination = match.group(1).strip().split(maxsplit=1)[0].strip("<>")
            link = urlsplit(destination)
            if link.scheme in ("http", "https", "mailto"):
                continue
            if link.scheme or link.netloc:
                raise SystemExit(f"Unsupported manual link in {document}: {destination}")
            target = (document.parent / unquote(link.path)).resolve() if link.path else document
            if not target.is_relative_to(physical_root) or not target.is_file():
                raise SystemExit(f"Missing or external manual target in {document}: {destination}")


def citations(root):
    """Refuse a gate whose cited revisions or evidence digests no longer resolve."""
    from citations import evidence_findings, unreachable_revisions

    unreachable = unreachable_revisions(root)
    mismatched, absent, verified = evidence_findings(root)
    for finding in (*unreachable, *mismatched):
        print(f"citation: {finding}", file=sys.stderr)
    if unreachable or mismatched:
        raise SystemExit("Cited revisions or evidence digests no longer resolve")
    print(f"citations: {verified} digest(s) verified; {len(absent)} artifact(s) absent on this host",
          flush=True)
