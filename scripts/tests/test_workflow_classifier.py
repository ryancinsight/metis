"""Computed byte classification and real workflow command boundaries."""

import os
import pathlib
import subprocess
import sys
import tempfile
import unittest


SCRIPTS = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import workflow_gate


class ByteClassification(unittest.TestCase):
    def test_documented_formats_case_empty_records_and_unknown_bytes(self):
        for suffix in (b"md", b"mdx", b"rst", b"adoc"):
            for path in (b"README." + suffix, b"crates/guide." + suffix.upper()):
                with self.subTest(path=path):
                    self.assertEqual(workflow_gate.classify(path + b"\0"), {"code": False, "fuzz": False})
        for suffix in (b"json", b"jpg", b"jpeg", b"png", b"svg", b"webp", b"pdf"):
            for prefix, code in ((b"docs/", False), (b"src/", True)):
                with self.subTest(suffix=suffix, prefix=prefix):
                    self.assertEqual(workflow_gate.classify(prefix + b"asset." + suffix),
                                     {"code": code, "fuzz": False})
        cases = (
            (b"\0\0README.md\0\0LICENSE\0", False, False),
            (b".GITHOOKS/pre-push\0crates/LICENSE\0", False, False),
            (b"docs/unknown.bin\0", True, False),
            (b"docs/\xffunknown\nname\0", True, False),
            (b"fuzz/README.md\0", False, True),
            (b"Fuzz/source.rs\0", True, False),
            (b"README.md\0fuzz/source.rs\0fuzz/source.rs\0", True, True),
        )
        for changed, code, fuzz in cases:
            with self.subTest(changed=changed):
                self.assertEqual(workflow_gate.classify(changed), {"code": code, "fuzz": fuzz})


class CommandBoundary(unittest.TestCase):
    def test_actual_cli_reads_git_bytes_and_appends_exact_outputs(self):
        cases = (
            (b"docs/image.png\0README.MD\0", "code=false\nfuzz=false\n"),
            (b"fuzz/source.rs\0", "code=true\nfuzz=true\n"),
            (b"\xffunknown\nname\0LICENSE\0", "code=true\nfuzz=false\n"),
        )
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            changed = root / "metis-changed-files"
            output = root / "github-output"
            environment = os.environ.copy()
            environment.update(RUNNER_TEMP=directory, GITHUB_OUTPUT=str(output))
            for body, expected in cases:
                with self.subTest(body=body):
                    changed.write_bytes(body)
                    output.write_text("retained=1\n", encoding="utf-8")
                    result = subprocess.run([sys.executable, str(SCRIPTS / "workflow_gate.py")],
                                            env=environment, capture_output=True, text=True, timeout=10)
                    self.assertEqual((result.returncode, result.stdout, result.stderr), (0, "", ""))
                    self.assertEqual(output.read_text(encoding="utf-8"), "retained=1\n" + expected)
                    self.assertEqual(changed.read_bytes(), body)

    def test_actual_cli_rejects_missing_environment_without_writing_results(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "metis-changed-files").write_bytes(b"fuzz/source.rs\0")
            output = root / "github-output"
            for missing in ("RUNNER_TEMP", "GITHUB_OUTPUT"):
                with self.subTest(missing=missing):
                    output.write_text("retained=1\n", encoding="utf-8")
                    environment = os.environ.copy()
                    environment.update(RUNNER_TEMP=directory, GITHUB_OUTPUT=str(output))
                    del environment[missing]
                    result = subprocess.run([sys.executable, str(SCRIPTS / "workflow_gate.py")],
                                            env=environment, capture_output=True, text=True, timeout=10)
                    self.assertEqual((result.returncode, result.stdout), (1, ""))
                    self.assertIn(missing, result.stderr)
                    self.assertEqual(output.read_text(encoding="utf-8"), "retained=1\n")

    def test_actual_cli_rejects_absent_input_and_unwritable_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            changed = root / "metis-changed-files"
            output = root / "github-output"
            output.write_text("retained=1\n", encoding="utf-8")
            environment = os.environ.copy()
            environment.update(RUNNER_TEMP=directory, GITHUB_OUTPUT=str(output))
            absent = subprocess.run([sys.executable, str(SCRIPTS / "workflow_gate.py")],
                                    env=environment, capture_output=True, text=True, timeout=10)
            self.assertEqual((absent.returncode, absent.stdout), (1, ""))
            self.assertIn("metis-changed-files", absent.stderr)
            self.assertEqual(output.read_text(encoding="utf-8"), "retained=1\n")
            changed.write_bytes(b"fuzz/source.rs\0")
            environment["GITHUB_OUTPUT"] = directory
            invalid_output = subprocess.run([sys.executable, str(SCRIPTS / "workflow_gate.py")],
                                            env=environment, capture_output=True, text=True, timeout=10)
            self.assertEqual((invalid_output.returncode, invalid_output.stdout), (1, ""))
            self.assertIn("workflow classification rejected:", invalid_output.stderr)
            self.assertEqual(output.read_text(encoding="utf-8"), "retained=1\n")


if __name__ == "__main__":
    unittest.main()
