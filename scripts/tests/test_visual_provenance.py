"""Bind all canonical visual owners through the actual locked source closure."""

import hashlib
import json
import pathlib
import subprocess
import sys
import unittest


SCRIPTS = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import verify
import visual_tools
import visual_tools.codec
import visual_tools.contract
import visual_tools.difference
from cargo_overlay import isolated_cargo


class SourceClosure(unittest.TestCase):
    def test_actual_locked_workspace_records_every_new_visual_owner(self):
        with isolated_cargo(SCRIPTS.parent) as (working, environment):
            result = subprocess.run(
                ["cargo", "metadata", "--manifest-path", str(SCRIPTS.parent / "Cargo.toml"),
                 "--format-version", "1", "--no-deps", "--locked", "--offline"],
                cwd=working, env=environment, capture_output=True, text=True, timeout=30,
            )
        self.assertEqual(result.returncode, 0, result.stderr)
        metadata = json.loads(result.stdout)
        self.assertIn("metis-core", {package["name"] for package in metadata["packages"]})
        recorded = verify.source_state(metadata, [])
        owners = (
            SCRIPTS / "visual.py", SCRIPTS / "visual_tools" / "__init__.py",
            SCRIPTS / "visual_tools" / "codec.py", SCRIPTS / "visual_tools" / "contract.py",
            SCRIPTS / "visual_tools" / "difference.py", SCRIPTS / "visual_tools" / "design.md",
        )
        for owner in owners:
            with self.subTest(owner=owner.name):
                canonical = owner.read_bytes().replace(b"\r\n", b"\n").replace(b"\r", b"\n")
                self.assertEqual(recorded[str(owner.resolve())], hashlib.sha256(canonical).hexdigest())
        for module in (visual_tools, visual_tools.codec, visual_tools.contract, visual_tools.difference):
            with self.subTest(module=module.__name__):
                self.assertEqual(pathlib.Path(module.__file__).resolve().parent,
                                 (SCRIPTS / "visual_tools").resolve())


if __name__ == "__main__":
    unittest.main()
