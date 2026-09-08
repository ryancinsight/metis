"""Check the bounded mutation runner's scope and outcome accounting."""

import json
import pathlib
import shutil
import sys
import tempfile
import unittest


SCRIPTS = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import mutation  # noqa: E402


class MutationOutcomeTests(unittest.TestCase):
    """Keep viable-mutant scoring tied to cargo-mutants' report contract."""

    def write_outcomes(self, values):
        directory = pathlib.Path(tempfile.mkdtemp(prefix="metis-mutants-test-"))
        self.addCleanup(lambda: shutil.rmtree(directory))
        path = directory / "outcomes.json"
        path.write_text(json.dumps(values), encoding="utf-8")
        return path

    def test_score_excludes_unviable_mutants(self):
        summary = mutation.summarize_outcomes(self.write_outcomes({
            "total_mutants": 14,
            "caught": 5,
            "missed": 0,
            "timeout": 0,
            "unviable": 9,
            "cargo_mutants_version": mutation.TOOL_VERSION,
        }))
        self.assertEqual(summary["viable_mutants"], 5)
        self.assertEqual(summary["caught"], 5)
        self.assertEqual(summary["score"], 1.0)

    def test_summary_preserves_surviving_and_timeout_counts(self):
        summary = mutation.summarize_outcomes(self.write_outcomes({
            "total_mutants": 4,
            "caught": 1,
            "missed": 2,
            "timeout": 1,
            "unviable": 0,
            "cargo_mutants_version": mutation.TOOL_VERSION,
        }))
        self.assertEqual(summary["missed"], 2)
        self.assertEqual(summary["timeout"], 1)
        self.assertEqual(summary["viable_mutants"], 4)


class MutationCommandContractTests(unittest.TestCase):
    """Keep the committed command focused on real cross-package decoder tests."""

    @classmethod
    def setUpClass(cls):
        cls.source = (SCRIPTS / "mutation.py").read_text(encoding="utf-8")

    def test_tool_and_scope_are_pinned(self):
        for fragment in (
            'TOOL_VERSION = "27.1.0"',
            'PACKAGE = "metis-core"',
            'TEST_PACKAGE = "metis-ipc"',
            'EXAMINE = "decode"',
            'TEST_TIMEOUT_SECONDS = 30',
            'BUILD_TIMEOUT_SECONDS = 120',
            'SUITE_TIMEOUT_SECONDS = 300',
            '"--test-tool",\n        "nextest"',
            '"--cargo-arg=--locked"',
            '"--cargo-arg=--offline"',
            '"--no-shuffle"',
        ):
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, self.source)

    def test_runner_does_not_introduce_registry_credentials(self):
        for forbidden in (
            "CARGO_REGISTRY_TOKEN",
            "PYPI_TOKEN",
            "TWINE_PASSWORD",
            "private_key",
            "signing-key",
            "SSH_PRIVATE_KEY",
        ):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, self.source)


if __name__ == "__main__":
    unittest.main()
