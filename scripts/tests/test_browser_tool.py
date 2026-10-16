"""Real producer CLI contracts preserve Rust artifact and argument ownership."""
import hashlib
import json
import pathlib
import subprocess
import sys
import tomllib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]


class BrowserToolContracts(unittest.TestCase):
    def test_real_producer_reports_actual_rust_artifact_and_complete_streams(self):
        result = subprocess.run([sys.executable, str(ROOT / "scripts/browser.py"), "tool"],
                                cwd=ROOT, capture_output=True, text=True, encoding="utf-8", timeout=60)
        self.assertEqual(result.returncode, 0, result.stderr)
        value = json.loads(result.stdout)
        artifact = value["compiler_artifact"]
        executable = pathlib.Path(value["executable"])
        self.assertEqual(value["schema"], 1)
        self.assertEqual(executable, pathlib.Path(artifact["executable"]))
        self.assertEqual(value["executable_sha256"], hashlib.sha256(executable.read_bytes()).hexdigest())
        self.assertEqual(value["lock_sha256"], hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest())
        self.assertEqual(value["source_root"], str(ROOT.resolve()))
        self.assertEqual((artifact["reason"], artifact["target"]["name"], artifact["target"]["kind"]),
                         ("compiler-artifact", "metis", ["bin"]))
        self.assertEqual(pathlib.Path(artifact["manifest_path"]), ROOT / "crates/metis-cli/Cargo.toml")
        self.assertEqual(artifact["profile"]["test"], False)
        self.assertEqual(value["cargo_argv"][-2:], ["--manifest-path", str(ROOT / "Cargo.toml")])
        self.assertEqual(value["cargo_argv"][1:8], ["build", "--locked", "--package", "metis-cli",
                                                   "--bin", "metis", "--message-format=json-render-diagnostics"])
        self.assertTrue(pathlib.Path(value["cargo_cwd"]).is_absolute())
        self.assertIsInstance(value["rustup_toolchain"], str)
        self.assertGreater(len(value["rustup_toolchain"]), 0)
        self.assertIn("Cargo compiler stdout:\n", result.stderr)
        self.assertIn("Cargo compiler stderr:\n", result.stderr)
        cargo_stdout, cargo_stderr = result.stderr.removeprefix("Cargo compiler stdout:\n").split(
            "\nCargo compiler stderr:\n", 1)
        records = [json.loads(line) for line in cargo_stdout.splitlines()]
        self.assertEqual([record for record in records if record.get("reason") == "compiler-artifact"
                          and record.get("target", {}).get("name") == "metis"
                          and record.get("target", {}).get("kind") == ["bin"]], [artifact])
        self.assertEqual(records[-1], {"reason": "build-finished", "success": True})
        self.assertIn("Finished", cargo_stderr)
        self.assertEqual(tomllib.loads((ROOT / "crates/metis-cli/Cargo.toml").read_text())["package"]["name"],
                         "metis-cli")

    def test_tool_rejects_each_consumer_only_flag_before_compilation(self):
        for flag in ("--consumer-package", "--consumer-gallery"):
            with self.subTest(flag=flag):
                result = subprocess.run([sys.executable, str(ROOT / "scripts/browser.py"), "tool", flag, "payload"],
                                        cwd=ROOT, capture_output=True, text=True, encoding="utf-8", timeout=10)
                self.assertEqual(result.returncode, 2)
                self.assertEqual(result.stdout, "")
                self.assertIn("unrecognized arguments", result.stderr)
                self.assertIn(flag, result.stderr)
                self.assertNotIn("Cargo compiler stdout", result.stderr)


if __name__ == "__main__":
    unittest.main()
