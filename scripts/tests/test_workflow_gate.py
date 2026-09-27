"""Verify the required workflow status gate against job result values."""
import json
import os
import pathlib
import re
import subprocess
import sys
import tempfile
import unittest


REPOSITORY = pathlib.Path(__file__).resolve().parents[2]
WORKFLOW = REPOSITORY / ".github" / "workflows" / "ci.yml"


class RequiredGateTests(unittest.TestCase):
    """Keep the hosted aggregate status aligned with every workflow job."""

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
        script = re.search(
            r"(?ms)^          python3 - <<'PY'\n(?P<script>.*?)^          PY$",
            cls.block,
        )
        if script is None:
            raise AssertionError("the Metis gate result check is missing")
        cls.script = "\n".join(
            line[10:] for line in script.group("script").splitlines()
        )

    def test_gate_covers_every_job_and_runs_when_pull_request_is_ready(self):
        jobs = self.source.split("\njobs:\n", 1)[1]
        job_ids = re.findall(r"^  ([a-z][a-z0-9_-]*):$", jobs, re.MULTILINE)
        self.assertEqual(len(job_ids), len(set(job_ids)))
        self.assertIn("gate", job_ids)
        self.assertEqual(
            set(self.dependencies),
            set(job_ids) - {"gate"},
        )
        self.assertEqual(len(self.dependencies), len(set(self.dependencies)))
        self.assertIn("name: Metis gate", self.block)
        self.assertIn("if: always()", self.block)
        self.assertIn("permissions: {}", self.block)
        self.assertIn("timeout-minutes: 5", self.block)
        self.assertIn("METIS_GATE_NEEDS: " + "$" + "{{ toJSON(needs) }}", self.block)
        self.assertIn(
            "METIS_GATE_DRAFT: "
            + "$"
            + "{{ github.event_name == 'pull_request' && github.event.pull_request.draft || false }}",
            self.block,
        )
        self.assertIn(
            "types: [opened, synchronize, reopened, ready_for_review]",
            self.source,
        )

    def run_gate(self, results, *, draft=False):
        environment = os.environ.copy()
        environment["METIS_GATE_NEEDS"] = json.dumps(results)
        environment["METIS_GATE_DRAFT"] = str(draft).lower()
        return subprocess.run(
            [sys.executable, "-c", self.script],
            check=False,
            capture_output=True,
            env=environment,
            text=True,
            timeout=10,
        )

    def test_success_and_skipped_dependencies_pass(self):
        results = {job: {"result": "success"} for job in self.dependencies}
        success = self.run_gate(results)
        self.assertEqual(success.returncode, 0, success.stderr)
        self.assertIn("All required Metis jobs completed.", success.stdout)

        skipped = dict(results)
        skipped[sorted(self.dependencies)[0]] = {"result": "skipped"}
        self.assertEqual(self.run_gate(skipped).returncode, 0)

    def test_failed_or_cancelled_dependencies_and_drafts_fail(self):
        passed = {job: {"result": "success"} for job in self.dependencies}
        for state in ("failure", "cancelled"):
            with self.subTest(state=state):
                results = dict(passed)
                results["verify"] = {"result": state}
                outcome = self.run_gate(results)
                self.assertEqual(outcome.returncode, 1)
                self.assertIn("verify", outcome.stderr)

        draft = self.run_gate(passed, draft=True)
        self.assertEqual(draft.returncode, 1)
        self.assertIn("draft pull requests", draft.stderr)


    def test_required_workflow_uses_job_level_path_filtering(self):
        triggers = self.source.split("\npermissions:\n", 1)[0]
        self.assertNotIn("\n    paths:", triggers)
        self.assertNotIn("\n    paths-ignore:", triggers)
        self.assertIn("  changes:\n", self.source)
        self.assertIn("git diff --name-only -z", self.source)
        self.assertIn('range="$BASE_SHA...$HEAD_SHA"', self.source)
        self.assertIn('range="$BEFORE_SHA..$CURRENT_SHA"', self.source)
        self.assertIn("0000000000000000000000000000000000000000", self.source)
        self.assertIn("needs: changes", self.source)
        self.assertIn("needs.changes.outputs.code == 'true'", self.source)
        self.assertIn("needs: [changes,", self.block)

    def test_path_classifier_only_skips_the_three_root_report_files(self):
        match = re.search(
            r"(?ms)^          python3 - <<'PY'\n(?P<script>.*?)^          PY$",
            self.source,
        )
        if match is None:
            self.fail("the changed-path classifier is missing")
        script = "\n".join(
            line[10:] for line in match.group("script").splitlines()
        )
        cases = (
            ((b"README.md",), False),
            ((b"LICENSE",), False),
            ((b"CHANGELOG.md",), False),
            ((), False),
            ((b"deny.toml",), True),
            ((b"crates/metis-core/Cargo.toml",), True),
            ((b"docs/manual/browser.md",), True),
            ((b"README.md", b"crates/metis-core/src/lib.rs"), True),
            ((b"new-unknown-file",), True),
        )
        with tempfile.TemporaryDirectory() as directory:
            changed = pathlib.Path(directory) / "metis-changed-files"
            output = pathlib.Path(directory) / "github-output"
            for paths, expected in cases:
                with self.subTest(paths=paths):
                    changed.write_bytes(
                        b"\0".join(paths) + (b"\0" if paths else b"")
                    )
                    output.write_text("", encoding="utf-8")
                    environment = os.environ.copy()
                    environment["RUNNER_TEMP"] = directory
                    environment["GITHUB_OUTPUT"] = str(output)
                    result = subprocess.run(
                        [sys.executable, "-c", script],
                        check=False,
                        capture_output=True,
                        env=environment,
                        text=True,
                        timeout=10,
                    )
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(
                        output.read_text(encoding="utf-8").strip(),
                        "code=" + str(expected).lower(),
                    )



if __name__ == "__main__":
    unittest.main()
