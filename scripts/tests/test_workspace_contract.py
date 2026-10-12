"""Value and ambiguity contracts for the authoritative workspace lock."""

import pathlib
import subprocess
import sys
import tempfile
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
from verification import workspace
import browser


class WorkspaceLockTests(unittest.TestCase):
    def test_version_comes_from_the_named_package(self):
        with tempfile.TemporaryDirectory() as temporary:
            lock = pathlib.Path(temporary) / "Cargo.lock"
            for version in ("0.2.128", "0.2.129"):
                lock.write_text('version = 4\n[[package]]\nname = "other"\nversion = "1.0.151"\n'
                                f'[[package]]\nname = "wasm-bindgen"\nversion = "{version}"\n')
                self.assertEqual(workspace.package_version(lock, "wasm-bindgen"), version)
                self.assertEqual(workspace.package_version(lock, "other"), "1.0.151")

    def test_missing_unversioned_and_duplicate_packages_are_rejected(self):
        entry = '[[package]]\nname = "wasm-bindgen"\nversion = "0.2.129"\n'
        with tempfile.TemporaryDirectory() as temporary:
            lock = pathlib.Path(temporary) / "Cargo.lock"
            for body in ('version = 4\n', '[[package]]\nname = "wasm-bindgen"\n', entry + entry):
                lock.write_text(body)
                with self.subTest(body=body), self.assertRaisesRegex(ValueError, "exactly one versioned package"):
                    workspace.package_version(lock, "wasm-bindgen")

    def test_installed_loader_matches_the_real_workspace_lock(self):
        tool = browser.wasm_bindgen()
        actual = subprocess.check_output([tool, "--version"], text=True, timeout=30).strip()
        expected = workspace.package_version(SCRIPTS.parent / "Cargo.lock", "wasm-bindgen")
        self.assertEqual(actual, f"wasm-bindgen {expected}")


if __name__ == "__main__":
    unittest.main()
