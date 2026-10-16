"""Native preparation receipts hash actual opaque files, not semantic stand-ins."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from verification.windows import prepared


def limits(**changes):
    # Scaled analytical domains exercise the operation; Rust supplies production limits.
    value = {"schema": 1, "records": 16, "portable_path_bytes": 240,
             "manifest_bytes": 16384, "module_bytes": 512, "payload_bytes": 1024}
    return value | changes


def fixture(root):
    directory = root / "browser" / "windows-webview-prepared" / "owned 患者"
    directory.mkdir(parents=True)
    inventory = directory / "opaque inventory.bin"
    inventory.write_bytes(b"abc")
    nested = directory / "nested λ"
    nested.mkdir()
    (nested / "arbitrary bytes.dat").write_bytes(bytes(range(256)))
    return directory, inventory


class OriginalPreparationContracts(unittest.TestCase):
    def test_every_enumerated_node_retains_actual_native_stat_identity_and_kind(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, inventory = fixture(Path(temporary).resolve())
            actual = {name: (path, metadata)
                      for path, name, metadata in prepared._nodes(directory, limits())}
            self.assertEqual(set(actual), {inventory.name, "nested λ/arbitrary bytes.dat"})
            for name, (path, metadata) in actual.items():
                with self.subTest(path=name):
                    native = path.stat(follow_symlinks=False)
                    self.assertEqual((metadata.st_dev, metadata.st_ino, metadata.st_nlink),
                                     (native.st_dev, native.st_ino, native.st_nlink))
                    self.assertTrue(os.path.samestat(metadata, native))
                    self.assertTrue(stat.S_ISREG(metadata.st_mode))
                    self.assertEqual(getattr(metadata, "st_file_attributes", None),
                                     getattr(native, "st_file_attributes", None))
                    self.assertEqual(getattr(metadata, "st_reparse_tag", None),
                                     getattr(native, "st_reparse_tag", None))
            self.assertTrue(os.path.samestat(actual[inventory.name][1], inventory.stat()))

    def test_original_bytes_and_actual_inventory_identity_survive_recording(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, inventory = fixture(Path(temporary).resolve())
            names = [(inventory, directory)]
            if os.name == "nt":
                import nt
                names.extend(((Path(nt._getfinalpathname(str(inventory))), directory),
                              (inventory, Path(nt._getfinalpathname(str(directory))))))
            original = {path.relative_to(directory).as_posix(): path.read_bytes()
                        for path in directory.rglob("*") if path.is_file()}
            for reported, owner in names:
                with self.subTest(reported=reported, owner=owner):
                    receipt_path = prepared.record(reported, owner, limits())
                    receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
                    self.assertTrue(receipt_path.parent.samefile(directory.parent))
                    self.assertEqual(receipt["reported_inventory"], str(reported))
                    self.assertEqual(receipt["directory"], str(owner))
                    self.assertEqual(receipt["inventory"], {
                        "path": inventory.name, "bytes": 3,
                        "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"})
                    self.assertEqual(receipt["files"], [
                        {"path": name, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
                        for name, data in sorted(original.items())])
                    self.assertEqual({path.relative_to(directory).as_posix(): path.read_bytes()
                                      for path in directory.rglob("*") if path.is_file()}, original)
                    receipt_path.unlink()

    def test_inclusive_bounds_and_exclusive_receipt_preserve_prior_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, inventory = fixture(Path(temporary).resolve())
            paths = [path.relative_to(directory).as_posix() for path in directory.rglob("*")]
            actual = limits(records=len(paths), module_bytes=256, payload_bytes=259,
                            portable_path_bytes=max(len(path.encode("utf-8")) for path in paths))
            receipt = prepared.record(inventory, directory, actual)
            before = receipt.read_bytes()
            actual["manifest_bytes"] = len(before)
            receipt.unlink()
            self.assertEqual(prepared.record(inventory, directory, actual).read_bytes(), before)
            with self.assertRaises(FileExistsError) as rejected:
                prepared.record(inventory, directory, actual)
            self.assertEqual(rejected.exception.filename, str(receipt))
            self.assertIn("create receipt", rejected.exception.__notes__[0])
            self.assertEqual(receipt.read_bytes(), before)
            self.assertEqual(inventory.read_bytes(), b"abc")

    def test_each_rust_budget_rejects_without_creating_or_modifying_a_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory, inventory = fixture(Path(temporary).resolve())
            cases = ((limits(records=2), "record budget"),
                     (limits(portable_path_bytes=3), "path budget"),
                     (limits(module_bytes=255), "module budget"),
                     (limits(payload_bytes=258), "payload budget"),
                     (limits(manifest_bytes=1), "manifest budget"),
                     (limits(records=True), "positive integer"),
                     (limits(schema=2), "budget schema"),
                     (limits(extra=1), "budget schema"))
            for budget, cause in cases:
                with self.subTest(budget=budget):
                    with self.assertRaisesRegex(ValueError, cause):
                        prepared.record(inventory, directory, budget)
                    self.assertFalse((directory.parent / f"{directory.name}.json").exists())
                    self.assertEqual(inventory.read_bytes(), b"abc")
                    self.assertEqual((directory / "nested λ/arbitrary bytes.dat").read_bytes(), bytes(range(256)))

    def test_native_missing_path_error_and_wrong_owner_identity_remain_distinct(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            directory, inventory = fixture(root)
            missing = directory / "missing 患者.bin"
            with self.assertRaises(OSError) as observed:
                missing.resolve(strict=True)
            native = observed.exception
            with self.assertRaises(type(native)) as rejected:
                prepared.record(missing, directory, limits())
            error = rejected.exception
            self.assertEqual((error.errno, error.filename, getattr(error, "winerror", None), str(error)),
                             (native.errno, native.filename, getattr(native, "winerror", None), str(native)))
            self.assertIn("resolve candidate", error.__notes__[0])
            with self.assertRaises(OSError) as stat_observed:
                missing.stat(follow_symlinks=False)
            stat_native = stat_observed.exception
            with self.assertRaises(type(stat_native)) as metadata_failure:
                prepared._metadata(missing)
            actual = metadata_failure.exception
            self.assertEqual((actual.errno, actual.filename, getattr(actual, "winerror", None), str(actual)),
                             (stat_native.errno, stat_native.filename, getattr(stat_native, "winerror", None), str(stat_native)))
            self.assertEqual(actual.__notes__, [f"prepared evidence stat without following links: {missing!r}"])
            outside = root / "same bytes.bin"
            outside.write_bytes(inventory.read_bytes())
            self.assertFalse(outside.samefile(inventory))
            with self.assertRaisesRegex(ValueError, "outside its owned directory"):
                prepared.record(outside, directory, limits())
            self.assertEqual(outside.read_bytes(), b"abc")

    def test_real_linked_nodes_and_multiple_inventory_names_reject(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            directory, inventory = fixture(root)
            duplicate = directory / "other inventory name"
            duplicate.hardlink_to(inventory)
            self.assertTrue(duplicate.samefile(inventory))
            with self.assertRaisesRegex(ValueError, "one native inventory identity"):
                prepared.record(inventory, directory, limits())
            duplicate.unlink()
            outside = root / "outside"
            outside.mkdir()
            link = directory / "linked directory"
            if os.name == "nt":
                shell = os.environ.get("COMSPEC") or shutil.which("cmd.exe")
                self.assertIsNotNone(shell, "The actual Windows junction case requires cmd.exe")
                result = subprocess.run([shell, "/d", "/c", "mklink", "/J", str(link), str(outside)],
                                        capture_output=True, timeout=10)
                self.assertEqual(result.returncode, 0, (result.stdout, result.stderr))
            else:
                link.symlink_to(outside, target_is_directory=True)
            self.assertTrue(link.samefile(outside))
            with self.assertRaisesRegex(ValueError, "linked nodes"):
                prepared.record(inventory, directory, limits())
            self.assertEqual(inventory.read_bytes(), b"abc")


if __name__ == "__main__":
    unittest.main()
