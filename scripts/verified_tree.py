"""Find a pull-request run that already verified a landed tree.

A push to the default branch whose tree a pull-request run of the verification
workflow already passed does not need the Windows gate a second time. That run
uploads the marker artifact ``verified-tree-<tree>`` when every required job
succeeded; this module decides which uploaded markers a push run may trust.
A marker counts only when its run is a completed, successful ``pull_request``
run of the verification workflow whose head repository is this repository, so
a fork's run, another workflow, a push or schedule run, or a failed run never
skips the gate.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from process_tree import ProcessTreeTimeout, run as run_process_tree  # noqa: E402

WORKFLOW_PATH = ".github/workflows/ci.yml"
LISTING_PAGE_SIZE = 100
API_TIMEOUT_SECONDS = 60.0
_TREE = re.compile(r"[0-9a-f]{40}(?:[0-9a-f]{24})?")


class ArtifactApiError(RuntimeError):
    """The hosting API did not return a usable answer."""


def marker_name(tree: str) -> str:
    """Name of the artifact recording that ``tree`` passed the pull-request gate."""
    if _TREE.fullmatch(tree) is None:
        raise ValueError(f"not a git tree object id: {tree!r}")
    return f"verified-tree-{tree}"


def candidate_run_ids(
    artifacts: Sequence[Mapping[str, object]], tree: str, repository_id: int
) -> list[int]:
    """Run ids owning an unexpired marker for ``tree`` uploaded by this repository."""
    wanted = marker_name(tree)
    run_ids: list[int] = []
    for artifact in artifacts:
        run = artifact.get("workflow_run")
        if (
            artifact.get("name") == wanted
            and artifact.get("expired") is False
            and isinstance(run, Mapping)
            and run.get("repository_id") == repository_id
            and run.get("head_repository_id") == repository_id
            and type(run.get("id")) is int
        ):
            run_ids.append(run["id"])
    return run_ids


def run_passed_pull_request_gate(
    run: Mapping[str, object], repository_id: int, workflow_path: str = WORKFLOW_PATH
) -> bool:
    """Whether ``run`` is a completed, successful pull-request run of the workflow."""
    head = run.get("head_repository")
    return (
        run.get("path") == workflow_path
        and run.get("event") == "pull_request"
        and run.get("status") == "completed"
        and run.get("conclusion") == "success"
        and isinstance(head, Mapping)
        and head.get("id") == repository_id
    )


def _api(path: str, fields: Mapping[str, str]) -> object:
    command = ["gh", "api", "-X", "GET", path]
    for key, value in fields.items():
        command += ["-f", f"{key}={value}"]
    try:
        result = run_process_tree(command, timeout=API_TIMEOUT_SECONDS)
    except ProcessTreeTimeout as error:
        raise ArtifactApiError(f"`gh api {path}` exceeded {API_TIMEOUT_SECONDS:g}s") from error
    if result.returncode != 0:
        raise ArtifactApiError(f"`gh api {path}` failed: {result.stderr.strip()}")
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise ArtifactApiError(f"`gh api {path}` returned invalid JSON: {error}") from error


def tree_is_verified(repository: str, repository_id: int, tree: str) -> bool:
    """Whether a trusted pull-request run already verified ``tree``."""
    listing = _api(
        f"repos/{repository}/actions/artifacts",
        {"name": marker_name(tree), "per_page": str(LISTING_PAGE_SIZE)},
    )
    artifacts = listing.get("artifacts") if isinstance(listing, Mapping) else None
    if not isinstance(artifacts, list):
        raise ArtifactApiError("the artifact listing has no `artifacts` array")
    for run_id in candidate_run_ids(artifacts, tree, repository_id):
        run = _api(f"repos/{repository}/actions/runs/{run_id}", {})
        if isinstance(run, Mapping) and run_passed_pull_request_gate(run, repository_id):
            return True
    return False


def main(argv: Sequence[str] | None = None) -> int:
    """Write ``verified=true|false`` to ``$GITHUB_OUTPUT`` (or stdout) for a tree."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repository", required=True, help="owner/name")
    parser.add_argument("--repository-id", required=True, type=int)
    parser.add_argument("--tree", required=True, help="git tree object id of the pushed commit")
    arguments = parser.parse_args(argv)
    try:
        verified = tree_is_verified(arguments.repository, arguments.repository_id, arguments.tree)
    except ArtifactApiError as error:
        # A failed lookup runs the gate: the cost is minutes, never a skipped check.
        print(f"::warning::verified-tree lookup failed, the gate will run: {error}", flush=True)
        verified = False
    line = f"verified={str(verified).lower()}"
    destination = os.environ.get("GITHUB_OUTPUT")
    if destination:
        with open(destination, "a", encoding="utf-8") as output:
            output.write(line + "\n")
    print(line, flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
