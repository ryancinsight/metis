"""Plan integrity checks fail on broken item references and links."""

from __future__ import annotations

import pathlib
import tempfile
import unittest

import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import plan


class PlanIntegrityTests(unittest.TestCase):
    """Exercise the board validator against real and malformed plans."""

    def test_repository_plan_is_consistent(self):
        self.assertEqual(plan.check_plan(), [])

    def test_missing_dependency_and_link_are_reported(self):
        with tempfile.TemporaryDirectory(prefix="metis-plan-test-") as directory:
            root = pathlib.Path(directory)
            (root / "backlog.md").write_text(
                '<a id="METIS-TEST-001"></a>\n'
                "## METIS-TEST-001 — Fixture\n"
                "- Status: todo\n"
                "- Dependencies: METIS-MISSING-001\n"
                "- See [missing](docs/missing.md)\n",
                encoding="utf-8",
            )
            (root / "checklist.md").write_text(
                "- [fixture](backlog.md#METIS-TEST-001)\n", encoding="utf-8"
            )
            findings = plan.check_plan(root)
            self.assertTrue(any("unknown dependency METIS-MISSING-001" in value for value in findings))
            self.assertTrue(any("missing local link target" in value for value in findings))

    def test_external_stack_links_are_left_for_the_stack_guard(self):
        with tempfile.TemporaryDirectory(prefix="metis-plan-test-") as directory:
            root = pathlib.Path(directory)
            (root / "backlog.md").write_text(
                '<a id="METIS-TEST-001"></a>\n'
                "## METIS-TEST-001 — Fixture\n"
                "- Status: todo\n"
                "- See [Atlas](../../backlog.md#metis-unregistered-member)\n",
                encoding="utf-8",
            )
            (root / "checklist.md").write_text("", encoding="utf-8")
            self.assertEqual(plan.check_plan(root), [])

    def test_missing_local_anchor_is_reported(self):
        with tempfile.TemporaryDirectory(prefix="metis-plan-test-") as directory:
            root = pathlib.Path(directory)
            (root / "backlog.md").write_text(
                '<a id="METIS-TEST-001"></a>\n'
                "## METIS-TEST-001 — Fixture\n"
                "- Status: todo\n"
                "- See [manual](docs/manual.md#missing)\n",
                encoding="utf-8",
            )
            (root / "checklist.md").write_text("", encoding="utf-8")
            manual = root / "docs" / "manual.md"
            manual.parent.mkdir()
            manual.write_text("# Existing\n", encoding="utf-8")
            findings = plan.check_plan(root)
            self.assertTrue(any("missing local link anchor: missing" in value for value in findings))


if __name__ == "__main__":
    unittest.main()
