"""Verify the required Metis workflow status against job results and changed paths."""
import json
import os
import pathlib
import re
import shlex
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
        expected_jobs = re.search(
            r"(?m)^          METIS_GATE_EXPECTED_JOBS: '([^']+)'$",
            cls.block,
        )
        if expected_jobs is None:
            raise AssertionError("the Metis gate must declare its expected job set")
        cls.expected_jobs = tuple(json.loads(expected_jobs.group(1)))
        script = re.search(
            r"(?ms)^          python3 - <<'PY'\n(?P<script>.*?)^          PY$",
            cls.block,
        )
        if script is None:
            raise AssertionError("the Metis gate result check is missing")
        cls.script = "\n".join(
            line[10:] for line in script.group("script").splitlines()
        )
        classifier = re.search(
            r"(?ms)^          python3 - <<'PY'\n(?P<script>.*?)^          PY$",
            cls.source,
        )
        if classifier is None:
            raise AssertionError("the changed-path classifier is missing")
        cls.classifier = "\n".join(
            line[10:] for line in classifier.group("script").splitlines()
        )
        diff = re.search(
            r'(?m)^          git diff (?P<options>.*?) "\$range" > ',
            cls.source,
        )
        if diff is None:
            raise AssertionError("the changed-path Git diff is missing")
        cls.diff_options = tuple(shlex.split(diff.group("options")))

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
        self.assertEqual(set(self.dependencies), set(self.expected_jobs))
        self.assertIn("name: Metis gate", self.block)
        self.assertIn("if: always()", self.block)
        self.assertIn("permissions: {}", self.block)
        self.assertIn("timeout-minutes: 5", self.block)
        self.assertIn("METIS_GATE_NEEDS: " + chr(36) + "{{ toJSON(needs) }}", self.block)
        self.assertIn(
            "METIS_GATE_DRAFT: "
            + chr(36)
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
        environment["METIS_GATE_EXPECTED_JOBS"] = json.dumps(self.expected_jobs)
        environment["METIS_GATE_DRAFT"] = str(draft).lower()
        return subprocess.run(
            [sys.executable, "-c", self.script],
            check=False,
            capture_output=True,
            env=environment,
            text=True,
            timeout=10,
        )

    def outputs(self, changed_paths):
        with tempfile.TemporaryDirectory() as directory:
            changed = pathlib.Path(directory) / "metis-changed-files"
            output = pathlib.Path(directory) / "github-output"
            changed.write_bytes(changed_paths)
            output.write_text("", encoding="utf-8")
            environment = os.environ.copy()
            environment["RUNNER_TEMP"] = directory
            environment["GITHUB_OUTPUT"] = str(output)
            result = subprocess.run(
                [sys.executable, "-c", self.classifier],
                check=False,
                capture_output=True,
                env=environment,
                text=True,
                timeout=10,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            return dict(
                line.split("=", 1)
                for line in output.read_text(encoding="utf-8").splitlines()
            )

    def classify(self, changed_paths):
        return "code=" + self.outputs(changed_paths)["code"]

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

        incomplete = dict(passed)
        incomplete.pop("verify")
        for results in (incomplete, {}):
            with self.subTest(results=results):
                outcome = self.run_gate(results)
                self.assertEqual(outcome.returncode, 1)
                self.assertIn("job set", outcome.stderr)

        unknown = dict(passed)
        unknown["verify"] = {"result": "neutral"}
        outcome = self.run_gate(unknown)
        self.assertEqual(outcome.returncode, 1)
        self.assertIn("verify", outcome.stderr)

    def test_required_workflow_uses_job_level_path_filtering(self):
        triggers = self.source.split("\npermissions:\n", 1)[0]
        self.assertNotIn("\n    paths:", triggers)
        self.assertNotIn("\n    paths-ignore:", triggers)
        self.assertIn("  changes:\n", self.source)
        self.assertIn("git diff --name-only --no-renames -z", self.source)
        self.assertIn('range="$BASE_SHA...$HEAD_SHA"', self.source)
        self.assertIn('range="$BEFORE_SHA..$CURRENT_SHA"', self.source)
        self.assertIn("0000000000000000000000000000000000000000", self.source)
        self.assertIn("needs: changes", self.source)
        self.assertIn("needs.changes.outputs.code == 'true'", self.source)
        self.assertIn("needs: [changes,", self.block)

    def test_path_classifier_marks_documentation_and_boards_non_code(self):
        cases = (
            ((b"README.md",), False),
            ((b"crates/metis-core/README.md",), False),
            ((b"LICENSE",), False),
            ((b"CHANGELOG.md",), False),
            ((b"docs/manual/browser.md",), False),
            ((b"docs/manual/images/gallery.webp",), False),
            ((b"docs/manual/images/captures.json",), False),
            ((b"backlog.md",), False),
            ((b"backlog/METIS-001.md",), False),
            ((b"gap_audit.md",), False),
            ((), False),
            ((b"deny.toml",), True),
            ((b"crates/metis-core/Cargo.toml",), True),
            ((b"docs/build.py",), True),
            ((b"README.md", b"crates/metis-core/src/lib.rs"), True),
            ((b"new-unknown-file",), True),
        )
        for paths, expected in cases:
            with self.subTest(paths=paths):
                data = b"\0".join(paths) + (b"\0" if paths else b"")
                self.assertEqual(
                    self.classify(data),
                    "code=" + str(expected).lower(),
                )

    def test_push_looks_up_a_verified_tree_and_the_windows_gate_honors_it(self):
        changes = self.source.split("\n  changes:\n", 1)[1].split("\n  verify:\n", 1)[0]
        verify = self.source.split("\n  verify:\n", 1)[1].split("\n  workflow-lint:\n", 1)[0]
        self.assertIn("      actions: read", changes)
        self.assertIn("tree: ${{ steps.tree.outputs.tree }}", changes)
        self.assertIn("verified: ${{ steps.verified.outputs.verified }}", changes)
        lookup = changes.split("      - name: Look up a pull-request run", 1)[1].split(
            "\n      - name: Determine changed paths", 1
        )[0]
        self.assertIn("if: github.event_name == 'push'", lookup)
        self.assertIn("id: verified", lookup)
        self.assertIn("python3 scripts/verified_tree.py", lookup)
        self.assertIn('--tree "$TREE"', lookup)
        self.assertIn("(github.event_name != 'push' || needs.changes.outputs.verified != 'true')", verify)
        # Scheduled, dispatched and merge-group runs never read the marker.
        self.assertIn("github.event_name == 'workflow_dispatch' ||", verify)
        self.assertIn("github.event_name == 'merge_group' ||", verify)

    def test_gate_records_the_verified_tree_only_after_a_green_pull_request(self):
        marker = "verified-tree-${{ needs.changes.outputs.tree }}"
        steps = self.block.split("\n      - name: ")[1:]
        recording = [step for step in steps if marker in step or "verified-tree.txt" in step]
        self.assertEqual(len(recording), 2)
        for step in recording:
            with self.subTest(step=step.splitlines()[0]):
                self.assertIn(
                    "if: github.event_name == 'pull_request' && needs.verify.result == 'success'",
                    step,
                )
        # A step with no `if` implies success(), so the result check above it
        # has already passed before the marker is written.
        self.assertTrue(steps[0].startswith("Check required job results"))
        self.assertNotIn("always()", "".join(recording))
        self.assertIn(marker, "".join(recording))

    def test_fuzz_campaign_follows_the_harness_for_pull_requests(self):
        cases = (
            ((b"fuzz/Cargo.toml",), True),
            ((b"fuzz/seeds/typeface/one-rectangle.ttf",), True),
            ((b"README.md", b"fuzz/fuzz_targets/archive.rs"), True),
            ((b"crates/metis-core/src/lib.rs",), False),
            ((b"docs/fuzz/notes.md",), False),
            ((), False),
        )
        for paths, expected in cases:
            with self.subTest(paths=paths):
                data = b"\0".join(paths) + (b"\0" if paths else b"")
                self.assertEqual(self.outputs(data)["fuzz"], str(expected).lower())
        fuzz = self.source.split("\n  fuzz:\n", 1)[1].split("\n  browser-assets:\n", 1)[0]
        self.assertIn("needs: changes", fuzz)
        self.assertIn("needs.changes.outputs.fuzz == 'true'", fuzz)
        self.assertIn("github.event.pull_request.draft == false", fuzz)
        self.assertIn("github.event_name == 'schedule'", fuzz)
        self.assertIn("github.event_name == 'workflow_dispatch'", fuzz)

    def test_windows_and_semver_skip_documentation_only_changes(self):
        verify = self.source.split("\n  verify:\n", 1)[1].split(
            "\n  workflow-lint:\n", 1
        )[0]
        semver = self.source.split("\n  semver:\n", 1)[1].split(
            "\n  fuzz:\n", 1
        )[0]
        for name, job in (("Windows", verify), ("SemVer", semver)):
            with self.subTest(job=name):
                self.assertIn("needs: changes", job)
                self.assertIn("needs.changes.outputs.code == 'true'", job)
                self.assertIn("github.event.pull_request.draft == false", job)

    def test_rename_to_report_file_keeps_the_source_path_in_the_gate(self):
        with tempfile.TemporaryDirectory() as directory:
            repository = pathlib.Path(directory) / "repo"
            repository.mkdir()

            def git(*arguments, input_bytes=None):
                result = subprocess.run(
                    ["git", "-C", str(repository), *arguments],
                    check=True,
                    capture_output=True,
                    input=input_bytes,
                    timeout=10,
                )
                return result.stdout.strip()

            git("init", "--quiet", "-b", "main")
            git("config", "diff.renames", "true")
            blob = git("hash-object", "-w", "--stdin", input_bytes=b"fn main() {}\n")
            source_tree = git(
                "mktree",
                input_bytes=f"100644 blob {blob.decode('ascii')}\tsource.rs\n".encode(),
            )
            report_tree = git(
                "mktree",
                input_bytes=f"100644 blob {blob.decode('ascii')}\tREADME.md\n".encode(),
            )
            default_diff = git(
                "diff", "--name-only", "-z", source_tree, report_tree
            )
            self.assertEqual([path for path in default_diff.split(bytes([0])) if path], [b"README.md"])
            changed = git("diff", *self.diff_options, source_tree, report_tree)
            paths = set(path for path in changed.split(b"\0") if path)
            self.assertEqual(paths, {b"README.md", b"source.rs"})
            self.assertEqual(self.classify(changed), "code=true")


if __name__ == "__main__":
    unittest.main()
