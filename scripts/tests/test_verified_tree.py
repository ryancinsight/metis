"""Decide which pull-request markers let a push skip the Windows gate."""

from __future__ import annotations

import contextlib
import io
import os
import pathlib
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import verified_tree  # noqa: E402

REPOSITORY = "ryancinsight/metis"
REPOSITORY_ID = 1358432215
FORK_ID = 42
TREE = "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
OTHER_TREE = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"


def artifact(**overrides):
    value = {
        "name": f"verified-tree-{TREE}",
        "expired": False,
        "workflow_run": {
            "id": 7,
            "repository_id": REPOSITORY_ID,
            "head_repository_id": REPOSITORY_ID,
        },
    }
    value.update(overrides)
    return value


def run(**overrides):
    value = {
        "path": ".github/workflows/ci.yml",
        "event": "pull_request",
        "status": "completed",
        "conclusion": "success",
        "head_repository": {"id": REPOSITORY_ID},
    }
    value.update(overrides)
    return value


class MarkerSelectionTests(unittest.TestCase):
    """A marker is trusted only for its own tree, unexpired, from this repository."""

    def test_marker_name_is_the_tree_id(self):
        self.assertEqual(verified_tree.marker_name(TREE), f"verified-tree-{TREE}")

    def test_marker_name_rejects_anything_but_a_tree_object_id(self):
        for value in ("", "main", TREE.upper(), TREE[:-1], TREE + "0", f"{TREE}; rm"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                verified_tree.marker_name(value)

    def test_unexpired_marker_from_this_repository_yields_its_run(self):
        self.assertEqual(
            verified_tree.candidate_run_ids([artifact()], TREE, REPOSITORY_ID), [7]
        )

    def test_rejected_markers_yield_no_run(self):
        other_run = {"id": 7, "repository_id": REPOSITORY_ID, "head_repository_id": REPOSITORY_ID}
        cases = {
            "other tree": artifact(name=f"verified-tree-{OTHER_TREE}"),
            "unrelated artifact": artifact(name="metis-verification-7"),
            "expired": artifact(expired=True),
            "expiry unknown": artifact(expired=None),
            "fork head": artifact(workflow_run={**other_run, "head_repository_id": FORK_ID}),
            "foreign repository": artifact(workflow_run={**other_run, "repository_id": FORK_ID}),
            "no run": artifact(workflow_run=None),
            "run id not an integer": artifact(workflow_run={**other_run, "id": "7"}),
        }
        for label, candidate in cases.items():
            with self.subTest(label=label):
                self.assertEqual(
                    verified_tree.candidate_run_ids([candidate], TREE, REPOSITORY_ID), []
                )

    def test_only_the_trusted_markers_among_several_are_returned(self):
        trusted = artifact(workflow_run={"id": 9, "repository_id": REPOSITORY_ID,
                                         "head_repository_id": REPOSITORY_ID})
        listing = [artifact(expired=True), artifact(name=f"verified-tree-{OTHER_TREE}"), trusted]
        self.assertEqual(verified_tree.candidate_run_ids(listing, TREE, REPOSITORY_ID), [9])


class RunTrustTests(unittest.TestCase):
    """A run vouches for its marker only when it passed as a pull request."""

    def test_completed_successful_pull_request_run_is_trusted(self):
        self.assertTrue(verified_tree.run_passed_pull_request_gate(run(), REPOSITORY_ID))

    def test_every_other_run_is_rejected(self):
        cases = {
            "other workflow": run(path=".github/workflows/python-release.yml"),
            "push event": run(event="push"),
            "schedule event": run(event="schedule"),
            "still running": run(status="in_progress", conclusion=None),
            "queued": run(status="queued", conclusion=None),
            "failed": run(conclusion="failure"),
            "cancelled": run(conclusion="cancelled"),
            "skipped": run(conclusion="skipped"),
            "fork head": run(head_repository={"id": FORK_ID}),
            "no head repository": run(head_repository=None),
        }
        for label, candidate in cases.items():
            with self.subTest(label=label):
                self.assertFalse(
                    verified_tree.run_passed_pull_request_gate(candidate, REPOSITORY_ID)
                )


class LookupTests(unittest.TestCase):
    """The lookup queries the listing, then the run of each trusted marker."""

    def lookup(self, responses):
        calls = []

        def api(path, fields):
            calls.append((path, dict(fields)))
            return responses[path]

        with patch.object(verified_tree, "_api", api):
            verified = verified_tree.tree_is_verified(REPOSITORY, REPOSITORY_ID, TREE)
        return verified, calls

    def test_trusted_marker_with_passing_run_verifies_the_tree(self):
        verified, calls = self.lookup({
            f"repos/{REPOSITORY}/actions/artifacts": {"artifacts": [artifact()]},
            f"repos/{REPOSITORY}/actions/runs/7": run(),
        })
        self.assertTrue(verified)
        self.assertEqual(calls[0][1]["name"], f"verified-tree-{TREE}")

    def test_marker_whose_run_failed_does_not_verify_the_tree(self):
        verified, _ = self.lookup({
            f"repos/{REPOSITORY}/actions/artifacts": {"artifacts": [artifact()]},
            f"repos/{REPOSITORY}/actions/runs/7": run(conclusion="failure"),
        })
        self.assertFalse(verified)

    def test_a_later_passing_run_verifies_after_an_earlier_failed_one(self):
        second = artifact(workflow_run={"id": 8, "repository_id": REPOSITORY_ID,
                                        "head_repository_id": REPOSITORY_ID})
        verified, _ = self.lookup({
            f"repos/{REPOSITORY}/actions/artifacts": {"artifacts": [artifact(), second]},
            f"repos/{REPOSITORY}/actions/runs/7": run(conclusion="failure"),
            f"repos/{REPOSITORY}/actions/runs/8": run(),
        })
        self.assertTrue(verified)

    def test_no_trusted_marker_makes_no_run_query(self):
        verified, calls = self.lookup({
            f"repos/{REPOSITORY}/actions/artifacts": {"artifacts": [artifact(expired=True)]},
        })
        self.assertFalse(verified)
        self.assertEqual(len(calls), 1)

    def test_listing_without_an_artifacts_array_is_an_api_error(self):
        with self.assertRaises(verified_tree.ArtifactApiError):
            self.lookup({f"repos/{REPOSITORY}/actions/artifacts": {"message": "Not Found"}})


class CommandTests(unittest.TestCase):
    """The step output carries the verdict, and a failed lookup runs the gate."""

    def invoke(self, outcome):
        arguments = ["--repository", REPOSITORY, "--repository-id", str(REPOSITORY_ID),
                     "--tree", TREE]
        with tempfile.TemporaryDirectory() as directory:
            destination = pathlib.Path(directory, "output")
            stdout = io.StringIO()
            with patch.dict(os.environ, {"GITHUB_OUTPUT": str(destination)}), \
                    patch.object(verified_tree, "tree_is_verified", outcome), \
                    contextlib.redirect_stdout(stdout):
                status = verified_tree.main(arguments)
            return status, destination.read_text(encoding="utf-8"), stdout.getvalue()

    def test_verified_tree_is_reported_as_true(self):
        status, output, _ = self.invoke(lambda *_: True)
        self.assertEqual((status, output), (0, "verified=true\n"))

    def test_unverified_tree_is_reported_as_false(self):
        status, output, _ = self.invoke(lambda *_: False)
        self.assertEqual((status, output), (0, "verified=false\n"))

    def test_api_failure_reports_false_and_surfaces_a_warning(self):
        def fail(*_):
            raise verified_tree.ArtifactApiError("`gh api` failed: HTTP 403")

        status, output, printed = self.invoke(fail)
        self.assertEqual((status, output), (0, "verified=false\n"))
        self.assertIn("::warning::verified-tree lookup failed, the gate will run:", printed)
        self.assertIn("HTTP 403", printed)


if __name__ == "__main__":
    unittest.main()
