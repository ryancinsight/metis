"""Check the resource instrument's schema and bounded sampling contract."""
import json
import os
import pathlib
import runpy
import sys
import tempfile
import unittest


SCRIPTS = pathlib.Path(__file__).resolve().parents[1]


class ResourceSummaryTests(unittest.TestCase):
    """Keep aggregation value-semantic and explicit about missing metrics."""

    @classmethod
    def setUpClass(cls):
        cls.resource = runpy.run_path(str(SCRIPTS / "resource.py"))

    def test_summary_reports_initial_peak_and_growth(self):
        sample = self.resource["ProcessSample"]
        result = self.resource["summarize"](
            (
                sample(2, 1, 100, 80, 7),
                sample(8, 1, 140, 95, 9),
                sample(14, 2, 125, 110, 8),
            )
        )
        self.assertEqual(result["sample_count"], 3)
        self.assertEqual(result["working_set_bytes"], {
            "initial": 100, "final": 125, "peak": 140, "growth": 25,
        })
        self.assertEqual(result["private_bytes"]["peak"], 110)
        self.assertEqual(result["process_count"], {"initial": 1, "peak": 2})

    def test_summary_preserves_unavailable_metric(self):
        sample = self.resource["ProcessSample"]
        result = self.resource["summarize"]((sample(1, 1, 100, None, None),))
        self.assertEqual(result["private_bytes"], {
            "initial": None, "final": None, "peak": None, "growth": None,
        })
        self.assertEqual(result["handle_count"]["peak"], None)

    def test_fingerprint_does_not_expose_arguments(self):
        fingerprint = self.resource["command_fingerprint"]
        command = ["viewer.exe", r"C:\private\patient-study", "1.2.840.999"]
        digest = fingerprint(command)
        self.assertEqual(len(digest), 64)
        self.assertNotIn("patient-study", digest)
        self.assertNotEqual(digest, fingerprint([*command, "changed"]))

    def test_report_rejects_hardlinked_destination(self):
        resource = self.resource
        with tempfile.TemporaryDirectory(prefix="metis-resource-link-") as directory:
            original = pathlib.Path(directory) / "original.json"
            alias = pathlib.Path(directory) / "alias.json"
            original.write_text("{}", encoding="utf-8")
            try:
                os.link(original, alias)
            except OSError as error:
                self.skipTest(f"hard links unavailable: {error}")
            with self.assertRaisesRegex(ValueError, "unsafe resource report path"):
                resource["_validate_output"](alias)


class ResourceProcessTests(unittest.TestCase):
    """Exercise a real child process through the public command runner."""

    def test_short_lifecycle_records_real_process_observations(self):
        resource = runpy.run_path(str(SCRIPTS / "resource.py"))
        with tempfile.TemporaryDirectory(prefix="metis-resource-test-") as directory:
            output = pathlib.Path(directory) / "report.json"
            result = resource["main"]([
                "--output", str(output), "--label", "test child", "--sample-ms", "20",
                "--timeout-seconds", "5", "--", sys.executable, "-c",
                "import time; time.sleep(0.12)",
            ])
            self.assertEqual(result, 0)
            report = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(report["schema"], 1)
            self.assertEqual(report["status"], "passed")
            measurement = report["measurements"][0]
            self.assertEqual(measurement["exit_code"], 0)
            self.assertGreaterEqual(measurement["summary"]["sample_count"], 2)
            self.assertEqual(report["argument_count"], 2)
            self.assertEqual(measurement["output_capture"], "disabled")

    def test_timeout_terminates_only_the_launched_process(self):
        resource = runpy.run_path(str(SCRIPTS / "resource.py"))
        with tempfile.TemporaryDirectory(prefix="metis-resource-timeout-") as directory:
            output = pathlib.Path(directory) / "report.json"
            result = resource["main"]([
                "--output", str(output), "--label", "timeout child", "--sample-ms", "20",
                "--timeout-seconds", "0.1", "--", sys.executable, "-c",
                "import time; time.sleep(5)",
            ])
            self.assertEqual(result, 1)
            report = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(report["status"], "timeout")
            measurement = report["measurements"][0]
            self.assertEqual(measurement["timeout_seconds"], 0.1)
            self.assertLess(measurement["duration_ms"], 5_000)

    def test_repeat_records_baseline_spread(self):
        resource = runpy.run_path(str(SCRIPTS / "resource.py"))
        with tempfile.TemporaryDirectory(prefix="metis-resource-repeat-") as directory:
            output = pathlib.Path(directory) / "report.json"
            result = resource["main"]([
                "--output", str(output), "--label", "repeated child", "--sample-ms", "20",
                "--timeout-seconds", "5", "--repeat", "3", "--", sys.executable, "-c",
                "import time; time.sleep(0.06)",
            ])
            self.assertEqual(result, 0)
            report = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(report["repeat"], 3)
            self.assertEqual(report["aggregate"]["run_count"], 3)
            self.assertEqual(report["aggregate"]["scalars"]["duration_ms"]["count"], 3)
            self.assertGreaterEqual(report["aggregate"]["scalars"]["duration_ms"]["mean"], 0)
