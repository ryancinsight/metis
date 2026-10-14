"""Value-semantic tests for running Cargo outside an inherited stack configuration."""
from __future__ import annotations

import os
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

from cargo_overlay import (
    configuration_directories,
    inherited_target_directory,
    isolated_cargo,
)


ROOT = pathlib.Path(__file__).resolve().parents[2]


def _require_clean_root(test: unittest.TestCase, directory: pathlib.Path) -> None:
    """Skip unless no ancestor of `directory` carries a Cargo configuration.

    A machine-wide `~/.cargo/config.toml` would otherwise decide these cases.
    """
    found = configuration_directories(directory)
    if found:
        test.skipTest(f"test directory inherits Cargo configuration: {found}")


class CargoOverlayTests(unittest.TestCase):
    def test_neutral_build_preserves_the_real_pinned_compiler(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.97.0"\n')
            (root / ".cargo").mkdir()
            (root / ".cargo/config.toml").write_text('[build]\ntarget-dir = "shared"\n')
            with mock.patch.dict(os.environ, {}, clear=False):
                os.environ.pop("RUSTUP_TOOLCHAIN", None)
                with isolated_cargo(root) as (working, environment):
                    self.assertNotEqual(working, root)
                    self.assertEqual(environment["RUSTUP_TOOLCHAIN"], "1.97.0")
                    actual = subprocess.check_output(["rustc", "-Vv"], cwd=working, env=environment,
                                                     text=True, timeout=30)
                    self.assertIn("release: 1.97.0\n", actual)
                os.environ["RUSTUP_TOOLCHAIN"] = "stable"
                with isolated_cargo(root) as (_, environment):
                    self.assertEqual(environment["RUSTUP_TOOLCHAIN"], "stable")

    def test_environment_target_directory_wins(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            configured = root / "elsewhere"
            with mock.patch.dict(os.environ, {"CARGO_TARGET_DIR": str(configured)}):
                self.assertEqual(inherited_target_directory(root), configured.resolve())

    def test_nearest_ancestor_target_directory_wins(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            _require_clean_root(self, root)
            (root / ".cargo").mkdir()
            (root / ".cargo" / "config.toml").write_text(
                '[build]\ntarget-dir = "shared"\n', encoding="utf-8"
            )
            member = root / "crates" / "inner"
            member.mkdir(parents=True)
            (member / ".cargo").mkdir()
            (member / ".cargo" / "config.toml").write_text(
                '[build]\ntarget-dir = "nearest"\n', encoding="utf-8"
            )
            with mock.patch.dict(os.environ, {}, clear=False):
                os.environ.pop("CARGO_TARGET_DIR", None)
                self.assertEqual(inherited_target_directory(member), (member / "nearest").resolve())
                self.assertEqual(inherited_target_directory(root), (root / "shared").resolve())

    def test_isolated_cargo_is_in_place_without_inherited_configuration(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            _require_clean_root(self, root)
            with mock.patch.dict(os.environ, {}, clear=False):
                os.environ.pop("CARGO_TARGET_DIR", None)
                with isolated_cargo(root) as (working, environment):
                    self.assertEqual(working, root)
                    self.assertNotIn("CARGO_TARGET_DIR", environment)

    def test_isolated_cargo_leaves_the_configuration_chain_but_keeps_the_cache(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            _require_clean_root(self, root)
            (root / ".cargo").mkdir()
            (root / ".cargo" / "config").write_text(
                '[patch."https://github.com/ryancinsight/Moirai"]\n'
                'moirai = { path = "local/moirai" }\n'
                '[build]\ntarget-dir = "shared"\n',
                encoding="utf-8",
            )
            with mock.patch.dict(os.environ, {}, clear=False):
                os.environ.pop("CARGO_TARGET_DIR", None)
                with isolated_cargo(root) as (working, environment):
                    self.assertNotEqual(working, root)
                    self.assertEqual(configuration_directories(working), [])
                    self.assertFalse(working.is_relative_to(root))
                    self.assertEqual(environment["CARGO_TARGET_DIR"], str((root / "shared").resolve()))

    def test_configuration_directories_are_outermost_first(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / ".cargo").mkdir()
            (root / ".cargo" / "config.toml").write_text("[build]\n", encoding="utf-8")
            member = root / "member"
            (member / ".cargo").mkdir(parents=True)
            (member / ".cargo" / "config.toml").write_text("[build]\n", encoding="utf-8")
            found = configuration_directories(member)
            expected = [root / ".cargo" / "config.toml", member / ".cargo" / "config.toml"]
            self.assertEqual(found[-2:], expected, "lower configurations must follow the member's own")

    def test_browser_build_routes_cargo_through_the_redirect(self):
        result = subprocess.run([sys.executable, str(ROOT / "scripts/browser.py"), "tool"],
                                cwd=ROOT, check=True, capture_output=True, text=True,
                                encoding="utf-8", timeout=60)
        value = json.loads(result.stdout)
        self.assertEqual(value["cargo_argv"][-2:], ["--manifest-path", str(ROOT / "Cargo.toml")])
        self.assertIn("--locked", value["cargo_argv"])
        directory = pathlib.Path(value["cargo_cwd"])
        if configuration_directories(ROOT):
            self.assertEqual(configuration_directories(directory), [])
            self.assertFalse(directory.is_relative_to(ROOT))
        else:
            self.assertEqual(directory, ROOT)
        target = inherited_target_directory(ROOT)
        if target is not None:
            self.assertTrue(pathlib.Path(value["executable"]).resolve().is_relative_to(target.resolve()))
        self.assertEqual(pathlib.Path(value["compiler_artifact"]["manifest_path"]),
                         ROOT / "crates/metis-cli/Cargo.toml")


if __name__ == "__main__":
    unittest.main()
