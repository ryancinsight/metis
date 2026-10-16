"""Computed stdlib result parity and real CLI contracts for timed discovery."""
import contextlib
import io
import json
import pathlib
import re
import subprocess
import sys
import tempfile
import types
import unittest

from scripts.verification.suite import TimedTextTestRunner


ROOT = pathlib.Path(__file__).resolve().parents[2]
PREFIX = "unittest-timing: "


def records(text):
    return [json.loads(line.removeprefix(PREFIX)) for line in text.splitlines()
            if line.startswith(PREFIX)]


def scenario():
    class Values(unittest.TestCase):
        def test_success(self):
            self.assertEqual(sum(range(10)), 45)

        def test_failure(self):
            self.assertEqual(sum(range(10)), 46)

        def test_error(self):
            int("3.5")

        @unittest.skip("explicit runner outcome vector")
        def test_skip(self):
            self.assertEqual(sum(range(10)), 45)

        @unittest.expectedFailure
        def test_expected_failure(self):
            self.assertEqual(sum(range(10)), 46)

        @unittest.expectedFailure
        def test_unexpected_success(self):
            self.assertEqual(sum(range(10)), 45)

        def test_subtests(self):
            for length, expected in ((3, 3), (4, 6), (5, 11)):
                with self.subTest(length=length):
                    self.assertEqual(sum(range(length)), expected)

    return unittest.defaultTestLoader.loadTestsFromTestCase(Values)


def outcomes(result):
    return {"tests_run": result.testsRun,
            "failures": [(case.id(), error.splitlines()[-1]) for case, error in result.failures],
            "errors": [(case.id(), error.splitlines()[-1]) for case, error in result.errors],
            "skips": [(case.id(), reason) for case, reason in result.skipped],
            "expected_failures": [(case.id(), error.splitlines()[-1])
                                  for case, error in result.expectedFailures],
            "unexpected_successes": [case.id() for case in result.unexpectedSuccesses],
            "success": result.wasSuccessful()}


class ObservedStream(io.StringIO):
    """Retain the actual complete timing records visible at each flush."""
    def __init__(self):
        super().__init__()
        self.flushed_records = []

    def flush(self):
        lines = self.getvalue().splitlines()
        if lines and lines[-1].startswith(PREFIX):
            self.flushed_records.extend(records(lines[-1]))
        super().flush()


class TimedSuiteContracts(unittest.TestCase):
    def test_computed_result_subtests_and_text_equal_stdlib(self):
        standard_stream, timed_stream = io.StringIO(), ObservedStream()
        standard = unittest.TextTestRunner(stream=standard_stream, verbosity=2).run(scenario())
        timed = TimedTextTestRunner(stream=timed_stream, verbosity=2).run(scenario())
        self.assertEqual(outcomes(timed), outcomes(standard))
        self.assertEqual((timed.testsRun, len(timed.failures), len(timed.errors),
                          len(timed.skipped), len(timed.expectedFailures),
                          len(timed.unexpectedSuccesses)), (7, 2, 1, 1, 1, 1))
        actual = records(timed_stream.getvalue())
        self.assertEqual(len(actual), 14)
        self.assertEqual(timed_stream.flushed_records, actual)
        for start, end in zip(actual[::2], actual[1::2]):
            self.assertEqual((start["event"], end["event"]), ("case_started", "case_finished"))
            self.assertEqual(start["test"], end["test"])
            self.assertGreaterEqual(end["monotonic_ns"], start["monotonic_ns"])
            self.assertEqual(end["duration_ns"], end["monotonic_ns"] - start["monotonic_ns"])
        self.assertEqual(actual[-1]["result_counts"], {
            "tests_run": 7, "failures": 2, "errors": 1, "skips": 1,
            "expected_failures": 1, "unexpected_successes": 1,
        })
        human = [line for line in timed_stream.getvalue().splitlines()
                 if line.startswith("test_")]
        self.assertEqual(human, [line for line in standard_stream.getvalue().splitlines()
                                 if line.startswith("test_")])
        for verbosity in (0, 1):
            baseline_stream, observed_stream = io.StringIO(), io.StringIO()
            baseline = unittest.TextTestRunner(stream=baseline_stream, verbosity=verbosity).run(scenario())
            observed = TimedTextTestRunner(stream=observed_stream, verbosity=verbosity).run(scenario())
            self.assertEqual(outcomes(observed), outcomes(baseline))
            self.assertEqual(records(observed_stream.getvalue()), [])
            self.assertEqual(re.sub(r"Ran 7 tests in [0-9.]+s", "Ran 7 tests", observed_stream.getvalue()),
                             re.sub(r"Ran 7 tests in [0-9.]+s", "Ran 7 tests", baseline_stream.getvalue()))
        exit_codes = []
        for runner in (unittest.TextTestRunner, TimedTextTestRunner):
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as stopped:
                unittest.main(module=types.ModuleType("empty"), argv=["unittest"],
                              testRunner=runner, testLoader=unittest.TestLoader())
            exit_codes.append(stopped.exception.code)
        self.assertEqual(exit_codes[1], exit_codes[0])
        class FailedFixture(unittest.TestCase):
            @classmethod
            def setUpClass(cls):
                int("3.5")

            def test_calculation(self):
                self.assertEqual(sum(range(10)), 45)

        fixture_results = []
        for runner in (unittest.TextTestRunner, TimedTextTestRunner):
            result = runner(stream=io.StringIO()).run(
                unittest.defaultTestLoader.loadTestsFromTestCase(FailedFixture))
            fixture_results.append(outcomes(result))
        self.assertEqual(fixture_results[1], fixture_results[0])
        self.assertEqual((fixture_results[1]["tests_run"], len(fixture_results[1]["errors"])), (0, 1))

    def invoke(self, failing):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            selected = root / "selected"
            selected.mkdir()
            (selected / "__init__.py").write_text("", encoding="utf-8")
            (root / "arithmetic.py").write_text("VALUE = sum(range(10))\n", encoding="utf-8")
            source = '''
import pathlib
import unittest
import scripts
from arithmetic import VALUE
class Values(unittest.TestCase):
    def test_computed_subtests(self):
        self.assertEqual(pathlib.Path(scripts.__file__).resolve(), pathlib.Path(__OWNER__))
        for divisor in (3, 5, 9):
            with self.subTest(divisor=divisor):
                self.assertEqual(VALUE % divisor, 0)
    @unittest.skip('explicit CLI outcome vector')
    def test_skip(self):
        self.assertEqual(VALUE, 45)
'''.replace("__OWNER__", repr(str(ROOT / "scripts" / "__init__.py")))
            if failing:
                source += '''
    def test_error(self):
        int('3.5')
    def test_failure(self):
        self.assertEqual(VALUE, 46)
'''
            (selected / "case_vectors.py").write_text(source, encoding="utf-8")
            (selected / "test_excluded.py").write_text(
                "raise AssertionError('explicit discovery pattern excludes this module')\n",
                encoding="utf-8")
            process = subprocess.run(
                [sys.executable, "-m", "scripts.verification.suite", "discover", "-s",
                 str(selected), "-p", "case_vectors.py", "-t", str(root), "-v"],
                cwd=ROOT, capture_output=True, text=True, encoding="utf-8", timeout=10)
        self.assertEqual(process.stdout, "")
        self.assertEqual(process.returncode, int(failing))
        actual = records(process.stderr)
        self.assertEqual([event["test"].split(".")[-1] for event in actual[::2]],
                         ["test_computed_subtests", "test_error", "test_failure", "test_skip"]
                         if failing else ["test_computed_subtests", "test_skip"])
        for start, end in zip(actual[::2], actual[1::2]):
            self.assertEqual((start["event"], end["event"]), ("case_started", "case_finished"))
            self.assertTrue(start["test"].startswith("selected.case_vectors.Values."))
            self.assertEqual(start["test"], end["test"])
            self.assertEqual(end["duration_ns"], end["monotonic_ns"] - start["monotonic_ns"])
            self.assertGreaterEqual(end["duration_ns"], 0)
        return actual[-1]["result_counts"], process.stderr

    def test_real_cli_preserves_success_skip_discovery_and_import_domain(self):
        counts, text = self.invoke(False)
        self.assertEqual(counts, {"tests_run": 2, "failures": 0, "errors": 0,
                                 "skips": 1, "expected_failures": 0, "unexpected_successes": 0})
        self.assertIn("OK (skipped=1)", text)

    def test_real_cli_preserves_failure_error_subtests_and_exit(self):
        counts, text = self.invoke(True)
        self.assertEqual(counts, {"tests_run": 4, "failures": 1, "errors": 1,
                                 "skips": 1, "expected_failures": 0, "unexpected_successes": 0})
        self.assertIn("ValueError: invalid literal for int() with base 10: '3.5'", text)
        self.assertIn("AssertionError: 45 != 46", text)
        self.assertIn("FAILED (failures=1, errors=1, skipped=1)", text)


if __name__ == "__main__":
    unittest.main()
