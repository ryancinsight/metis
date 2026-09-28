"""Verify the aggregate Metis workflow gate and its path classifier."""

import pathlib
import re
import unittest


REPOSITORY = pathlib.Path(__file__).resolve().parents[2]
WORKFLOW = REPOSITORY / ".github" / "workflows" / "ci.yml"


class RequiredGateTests(unittest.TestCase):
    """Keep required workflow jobs represented by one aggregate status."""

    @classmethod
    def setUpClass(cls):
        cls.source = WORKFLOW.read_text(encoding="utf-8")
        jobs = cls.source.split("\njobs:\n", 1)[1]
        match = re.search(r"(?ms)^  gate:\n(?P<block>.*)\Z", jobs)
        if match is None:
            raise AssertionError("the required Metis gate job is missing")
        cls.block = match.group("block")
        needs = re.search(r"(?m)^    needs: \[([^\]]+)\]$", cls.block)
        if needs is None:
            raise AssertionError("the Metis gate job must declare every dependency")
        cls.dependencies = tuple(job.strip() for job in needs.group(1).split(","))

    def test_gate_covers_every_job_and_rejects_drafts(self):
        jobs = self.source.split("\njobs:\n", 1)[1]
        job_ids = re.findall(r"^  ([a-z][a-z0-9_-]*):$", jobs, re.MULTILINE)
        self.assertEqual(len(job_ids), len(set(job_ids)))
        self.assertEqual(set(self.dependencies), set(job_ids) - {"gate"})
        self.assertEqual(len(self.dependencies), len(set(self.dependencies)))
        self.assertIn("name: Metis gate", self.block)
        self.assertIn("if: always()", self.block)
        self.assertIn("Draft pull requests are never merge-ready", self.block)
        self.assertIn("exit 1", self.block)
        self.assertIn('select(.value.result == "failure" or .value.result == "cancelled")', self.block)

    def test_required_workflow_uses_job_level_path_classification(self):
        triggers = self.source.split("\npermissions:\n", 1)[0]
        self.assertNotIn("\n    paths:", triggers)
        self.assertNotIn("\n    paths-ignore:", triggers)
        self.assertIn("  changes:\n", self.source)
        self.assertIn("git diff --name-only --no-renames -z", self.source)
        self.assertIn('range="$BASE_SHA...$HEAD_SHA"', self.source)
        self.assertIn('range="$BEFORE_SHA..$CURRENT_SHA"', self.source)
        self.assertIn("documentation_asset_suffixes", self.source)
        self.assertIn("needs.changes.outputs.code == 'true'", self.source)


if __name__ == "__main__":
    unittest.main()
