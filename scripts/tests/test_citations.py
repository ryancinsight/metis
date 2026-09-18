"""Citation integrity fails on rewritten revisions and contradicted evidence."""

from __future__ import annotations

import hashlib
import pathlib
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
