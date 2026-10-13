"""Canonical changed-path classification for the hosted workflow.

For nonempty NUL-separated Git path records P, code = exists p in P outside
the documented build-inert path set, and fuzz = exists p prefixed by fuzz/.
Unknown byte paths therefore retain code admission without text decoding.

Classification is independent of the required-job admission owner in ci.yml.
The CLI reads the exact Git byte file and appends the two computed outputs.
"""

from __future__ import annotations

import os
from pathlib import Path
import sys


def classify(changed: bytes) -> dict[str, bool]:
    """Classify actual Git bytes; unknown paths affect the build by default."""
    paths = tuple(path for path in changed.split(b"\0") if path)
    markdown_suffixes = (b".md", b".mdx", b".rst", b".adoc")
    asset_suffixes = (b".json", b".jpg", b".jpeg", b".png", b".svg", b".webp", b".pdf")

    def build_inert(path: bytes) -> bool:
        lowered = path.lower()
        return (
            lowered.startswith(b".githooks/")
            or lowered.endswith(markdown_suffixes)
            or lowered == b"license"
            or lowered.endswith(b"/license")
            or (lowered.startswith(b"docs/") and lowered.endswith(asset_suffixes))
        )

    return {
        "code": any(not build_inert(path) for path in paths),
        "fuzz": any(path.startswith(b"fuzz/") for path in paths),
    }


def main() -> int:
    """Read the real workflow environment and owned input/output files once."""
    try:
        changed = Path(os.environ["RUNNER_TEMP"], "metis-changed-files").read_bytes()
        result = classify(changed)
        with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
            output.write("".join(f"{name}={str(value).lower()}\n" for name, value in result.items()))
    except (KeyError, OSError, ValueError) as error:
        print(f"workflow classification rejected: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
