"""Citation integrity fails on rewritten revisions and contradicted evidence."""

from __future__ import annotations

import hashlib
import os
import pathlib
import subprocess
import tempfile
import unittest
from unittest import mock

import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import citations


def workspace(directory):
    """Build a minimal checkout whose citations the checks can be pointed at."""
    root = pathlib.Path(directory)
    (root / "docs").mkdir()
    return root


class RevisionCitationTests(unittest.TestCase):
    """A cited revision must be reachable from the reference the gate checks."""

    def test_repository_citations_resolve(self):
        self.assertEqual(citations.unreachable_revisions(citations.ROOT), [])

    def test_batch_resolution_reports_local_commit_outside_reference_history(self):
        with tempfile.TemporaryDirectory(prefix="metis-citations-") as directory:
            root = workspace(directory)
            environment = {
                **os.environ,
                "GIT_AUTHOR_NAME": "Metis Test",
                "GIT_AUTHOR_EMAIL": "metis-test@example.invalid",
                "GIT_COMMITTER_NAME": "Metis Test",
                "GIT_COMMITTER_EMAIL": "metis-test@example.invalid",
            }

            def git(*arguments):
                return subprocess.run(
                    ["git", *arguments],
                    cwd=root,
                    env=environment,
                    check=True,
                    capture_output=True,
                    text=True,
                ).stdout.strip()

            git("init", "--quiet")
            (root / "README.md").write_text("Citation resolver fixture.\n", encoding="utf-8")
            git("add", "README.md")
            git("commit", "--quiet", "-m", "Create reachable citation")
            reachable = git("rev-parse", "HEAD")
            tree = git("rev-parse", "HEAD^{tree}")
            orphaned_first = git("commit-tree", tree, "-m", "First unreachable cited commit")
            orphaned_second = git("commit-tree", tree, "-m", "Second unreachable cited commit")
            (root / "backlog.md").write_text(
                f"- Reachable `{reachable}`.\n"
                f"- Second `{orphaned_second}`.\n"
                f"- First `{orphaned_first}`.\n",
                encoding="utf-8",
            )
            git("add", "backlog.md")
            git("commit", "--quiet", "-m", "Record revision citations")

            findings = citations.unreachable_revisions(root)

            self.assertEqual(len(findings), 2)
            self.assertIn(orphaned_second[:12], findings[0])
            self.assertIn("Second unreachable cited commit", findings[0])
            self.assertIn(orphaned_first[:12], findings[1])
            self.assertIn("First unreachable cited commit", findings[1])


class GitRevisionQueryTests(unittest.TestCase):
    """Live revision checks classify all candidates in bounded Git queries."""

    def test_batch_query_classifies_commits_and_ignores_other_objects(self):
        reachable = "a" * 40
        unreachable = "b" * 40
        blob = "c" * 40
        missing = "d" * 40
        candidates = (reachable[:12], unreachable[:12], blob[:12], missing, reachable[:12])
        batch_output = "\n".join(
            (
                f"{reachable} commit",
                f"{unreachable} commit",
                f"{blob} blob",
                f"{missing} missing",
            )
        )

        with mock.patch.object(citations, "_git", side_effect=[reachable, batch_output]) as git:
            resolved = citations._resolve_revisions(pathlib.Path("repo"), "HEAD", candidates)

        self.assertEqual(
            resolved,
            {
                reachable[:12]: True,
                unreachable[:12]: False,
                blob[:12]: None,
                missing: None,
            },
        )
        self.assertEqual(git.call_count, 2)
        self.assertEqual(git.call_args_list[0], mock.call(pathlib.Path("repo"), "rev-list", "HEAD"))
        self.assertEqual(
            git.call_args_list[1],
            mock.call(
                pathlib.Path("repo"),
                "cat-file",
                "--batch-check=%(objectname) %(objecttype)",
                input_text="".join(f"{candidate}\n" for candidate in candidates[:-1]),
            ),
        )

    def test_batch_query_rejects_incomplete_classification(self):
        with mock.patch.object(citations, "_git", side_effect=["", "a" * 40 + " commit"]):
            with self.assertRaisesRegex(citations.GitCommandError, "1 result.*2 candidate"):
                citations._resolve_revisions(pathlib.Path("repo"), "HEAD", ("a" * 12, "b" * 12))

    def test_git_query_reports_exit_diagnostic(self):
        failed = subprocess.CompletedProcess(
            ["git", "rev-list", "missing"], 128, stdout="", stderr="bad revision"
        )
        with mock.patch.object(subprocess, "run", return_value=failed) as run:
            with self.assertRaisesRegex(citations.GitCommandError, "exit code 128.*bad revision"):
                citations._git(pathlib.Path("repo"), "rev-list", "missing")
        self.assertEqual(run.call_args.kwargs["timeout"], citations.GIT_TIMEOUT_SECONDS)

    def test_git_query_reports_timeout(self):
        timeout = subprocess.TimeoutExpired(["git", "rev-list", "HEAD"], 10)
        with mock.patch.object(subprocess, "run", side_effect=timeout) as run:
            with self.assertRaisesRegex(citations.GitCommandError, "exceeded 10s"):
                citations._git(pathlib.Path("repo"), "rev-list", "HEAD")
        self.assertEqual(run.call_args.kwargs["timeout"], citations.GIT_TIMEOUT_SECONDS)

    def test_rewritten_revision_is_reported_with_its_location(self):
        with tempfile.TemporaryDirectory(prefix="metis-citations-") as directory:
            root = workspace(directory)
            landed = "a" * 40
            rewritten = "b" * 40
            (root / "backlog.md").write_text(
                f"- Landed at `{landed}`.\n"
                f"- The verifier passed at branch revision `{rewritten}`.\n",
                encoding="utf-8",
            )
            findings = citations.unreachable_revisions(
                root, "HEAD", resolve={landed: True, rewritten: False}.get
            )
            self.assertEqual(len(findings), 1)
            self.assertIn("backlog.md:2", findings[0])
            self.assertIn(rewritten[:12], findings[0])

    def test_another_repository_revision_is_ignored(self):
        with tempfile.TemporaryDirectory(prefix="metis-citations-") as directory:
            root = workspace(directory)
            foreign = "c" * 40
            (root / "backlog.md").write_text(
                f"- Moirai `{foreign}` publishes the shared HMAC contract.\n",
                encoding="utf-8",
            )
            self.assertEqual(citations.unreachable_revisions(root, "HEAD", resolve=lambda _: None), [])

    def test_digest_lines_are_not_read_as_revisions(self):
        with tempfile.TemporaryDirectory(prefix="metis-citations-") as directory:
            root = workspace(directory)
            digest = "d" * 64
            (root / "backlog.md").write_text(
                f"- Trace `output/run.json` with SHA-256 `{digest}`.\n",
                encoding="utf-8",
            )
            self.assertEqual(citations.unreachable_revisions(root, "HEAD", resolve=lambda _: False), [])

    def test_documents_inside_docs_are_scanned(self):
        with tempfile.TemporaryDirectory(prefix="metis-citations-") as directory:
            root = workspace(directory)
            rewritten = "e" * 40
            (root / "docs" / "manual.md").write_text(
                f"- Captured at revision `{rewritten}`.\n", encoding="utf-8"
            )
            findings = citations.unreachable_revisions(root, "HEAD", resolve=lambda _: False)
            self.assertEqual(len(findings), 1)
            self.assertIn("docs/manual.md:1", findings[0])


class EvidenceCitationTests(unittest.TestCase):
    """A present artifact must match the digest the documents pin for it."""

    def _cite(self, root, artifact, digest):
        (root / "backlog.md").write_text(
            f"- Trace `{artifact}` with SHA-256\n  `{digest}`.\n", encoding="utf-8"
        )

    def test_present_artifact_is_verified(self):
        with tempfile.TemporaryDirectory(prefix="metis-citations-") as directory:
            root = workspace(directory)
            artifact = root / "output" / "browser" / "runtime" / "trace.json"
            artifact.parent.mkdir(parents=True)
            artifact.write_bytes(b'{"ok": true}')
            self._cite(root, "output/browser/runtime/trace.json", hashlib.sha256(artifact.read_bytes()).hexdigest())
            self.assertEqual(citations.evidence_findings(root), ([], [], 1))

    def test_contradicted_digest_is_reported(self):
        with tempfile.TemporaryDirectory(prefix="metis-citations-") as directory:
            root = workspace(directory)
            artifact = root / "output" / "browser" / "runtime" / "trace.json"
            artifact.parent.mkdir(parents=True)
            artifact.write_bytes(b'{"ok": true}')
            self._cite(root, "output/browser/runtime/trace.json", "f" * 64)
            mismatched, absent, verified = citations.evidence_findings(root)
            self.assertEqual((absent, verified), ([], 0))
            self.assertEqual(len(mismatched), 1)
            self.assertIn("cited ffffffffffff", mismatched[0])

    def test_absent_artifact_is_reported_without_a_mismatch(self):
        with tempfile.TemporaryDirectory(prefix="metis-citations-") as directory:
            root = workspace(directory)
            self._cite(root, "output/browser/runtime/trace.json", "a" * 64)
            self.assertEqual(citations.evidence_findings(root), ([], ["backlog.md:1: output/browser/runtime/trace.json: cited artifact is absent on this host"], 0))

    def test_digest_of_another_artifact_in_the_sentence_is_not_bound(self):
        """A trace row that goes on to name a screenshot must not borrow its digest."""
        with tempfile.TemporaryDirectory(prefix="metis-citations-") as directory:
            root = workspace(directory)
            artifact = root / "output" / "browser" / "runtime" / "trace.json"
            artifact.parent.mkdir(parents=True)
            artifact.write_bytes(b'{"ok": true}')
            (root / "backlog.md").write_text(
                f"- The trace is `output/browser/runtime/trace.json` and its first PNG is 1875 by 1903"
                f" pixels with SHA-256 `{'f' * 64}`.\n",
                encoding="utf-8",
            )
            mismatched, absent, verified = citations.evidence_findings(root)
            self.assertEqual((mismatched, absent, verified), ([], [], 0))


class CitationExitStatusTests(unittest.TestCase):
    """Host state alone must not change the gate's exit status."""

    def _absent_workspace(self, directory):
        root = workspace(directory)
        (root / "backlog.md").write_text(
            f"- Trace `output/browser/runtime/trace.json` with SHA-256 `{'a' * 64}`.\n",
            encoding="utf-8",
        )
        return root

    def test_absent_artifact_passes_by_default_and_fails_when_required(self):
        with tempfile.TemporaryDirectory(prefix="metis-citations-") as directory:
            root = self._absent_workspace(directory)
            with mock.patch.object(sys, "argv", ["citations.py", "--root", str(root)]):
                self.assertEqual(citations.main(), 0)
            with mock.patch.object(sys, "argv", ["citations.py", "--root", str(root), "--require-artifacts"]):
                self.assertEqual(citations.main(), 1)

    def test_unreachable_revision_fails_the_gate(self):
        with tempfile.TemporaryDirectory(prefix="metis-citations-") as directory:
            root = self._absent_workspace(directory)
            (root / "backlog.md").write_text(
                f"- Captured at revision `{'b' * 40}`.\n", encoding="utf-8"
            )
            with mock.patch.object(sys, "argv", ["citations.py", "--root", str(root)]):
                with mock.patch.object(citations, "unreachable_revisions", return_value=["backlog.md:1: deadbeef"]):
                    self.assertEqual(citations.main(), 1)


if __name__ == "__main__":
    unittest.main()
