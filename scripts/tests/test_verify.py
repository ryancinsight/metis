"""Exercise gate bootstrap failures without invoking a Rust toolchain."""
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest


SCRIPTS = pathlib.Path(__file__).resolve().parents[1]


class BootstrapEvidenceTests(unittest.TestCase):
    """A failed new invocation must never retain an earlier green report."""

    def setUp(self):
        fixture = tempfile.TemporaryDirectory(prefix="metis-gate-test-")
        self.addCleanup(fixture.cleanup)
        self.root = pathlib.Path(fixture.name)
        scripts = self.root / "scripts"
        scripts.mkdir()
        for name in ("verify.py", "visual.py"):
            shutil.copyfile(SCRIPTS / name, scripts / name)
        (self.root / "Cargo.lock").write_text("version = 4\n", encoding="utf-8")
        (self.root / "rust-toolchain.toml").write_text(
            '[toolchain]\nchannel = "1.97.0"\n', encoding="utf-8"
        )
        output = self.root / "output"
        output.mkdir()
        self.report = output / "verification.json"
        self.previous = json.dumps({
            "schema": 1,
            "status": "passed",
            "stages": {"tests": "passed"},
            "revision": "previous-revision",
            "lock_sha256": "previous-lock",
            "visual": {"status": "passed"},
        }).encode("utf-8")
        self.report.write_bytes(self.previous)

    def invoke(self, *arguments):
        environment = os.environ.copy()
        environment["RUSTC"] = "deliberate-bootstrap-override"
        environment["PYTHONPATH"] = ""
        # A regression reaching tool execution cannot discover the host toolchain.
        environment["PATH"] = str(self.root / "unavailable-tools")
        return subprocess.run(
            [sys.executable, "-S", str(self.root / "scripts" / "verify.py"), *arguments],
            cwd=self.root,
            env=environment,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=10,
            check=False,
        )

    def assert_failed_report(self, result, diagnostic):
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn(diagnostic, result.stdout + result.stderr)
        report = json.loads(self.report.read_text(encoding="utf-8"))
        self.assertEqual(set(report), {"schema", "status", "stages", "commands", "error"})
        self.assertEqual(report["schema"], 1)
        self.assertEqual(report["status"], "failed")
        self.assertEqual(report["stages"], {})
        self.assertEqual(report["commands"], {})
        self.assertIn(report["error"], result.stdout + result.stderr)
        self.assertNotEqual(report["error"], "")

    def test_compiler_override_replaces_prior_success(self):
        result = self.invoke()
        self.assert_failed_report(result, "Unset RUSTC/RUSTDOC overrides")
        self.assertIn(
            "Unset RUSTC/RUSTDOC overrides",
            json.loads(self.report.read_text(encoding="utf-8"))["error"],
        )

    def test_malformed_toolchain_replaces_prior_success(self):
        (self.root / "rust-toolchain.toml").write_text("[toolchain\n", encoding="utf-8")
        self.assert_failed_report(self.invoke(), "TOMLDecodeError")

    def test_missing_visual_module_replaces_prior_success(self):
        (self.root / "scripts" / "visual.py").unlink()
        self.assert_failed_report(self.invoke(), "No module named 'visual'")

    def test_help_preserves_prior_report(self):
        result = self.invoke("--help")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("--update-snapshots", result.stdout)
        self.assertEqual(result.stderr, "")
        self.assertEqual(self.report.read_bytes(), self.previous)

    def outside_sentinel(self):
        outside = tempfile.TemporaryDirectory(prefix="metis-outside-test-")
        self.addCleanup(outside.cleanup)
        path = pathlib.Path(outside.name) / "verification.json"
        path.write_bytes(b"Unique outside work must survive a rejected gate invocation.\n")
        return path

    def assert_rejected_alias(self, sentinel):
        original = sentinel.read_bytes()
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Unsafe", result.stdout + result.stderr)
        self.assertEqual(sentinel.read_bytes(), original)

    def test_report_hardlink_preserves_outside_file(self):
        sentinel = self.outside_sentinel()
        self.report.unlink()
        os.link(sentinel, self.report)
        self.assertEqual(self.report.stat().st_nlink, 2)
        self.assert_rejected_alias(sentinel)

    def test_output_directory_link_preserves_outside_file(self):
        sentinel = self.outside_sentinel()
        output = self.report.parent
        self.report.unlink()
        output.rmdir()
        if os.name == "nt":
            # Junction creation needs no administrator symlink privilege. Paths
            # travel through environment values rather than PowerShell source.
            shell = shutil.which("powershell.exe")
            self.assertIsNotNone(shell, "Windows PowerShell creates the real junction fixture")
            environment = os.environ.copy()
            environment["METIS_TEST_LINK"] = str(output)
            environment["METIS_TEST_TARGET"] = str(sentinel.parent)
            result = subprocess.run(
                [shell, "-NoProfile", "-NonInteractive", "-Command",
                 "$ErrorActionPreference = 'Stop'; New-Item -ItemType Junction "
                 "-Path $env:METIS_TEST_LINK -Value $env:METIS_TEST_TARGET | Out-Null"],
                env=environment,
                capture_output=True,
                text=True,
                timeout=10,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.addCleanup(output.rmdir)
        else:
            output.symlink_to(sentinel.parent, target_is_directory=True)
            self.addCleanup(output.unlink)
        self.assertEqual(output.resolve(), sentinel.parent.resolve())
        self.assert_rejected_alias(sentinel)


if __name__ == "__main__":
    unittest.main()
