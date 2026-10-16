"""Existing native file containment uses actual directory identity."""

import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from verification.native_paths import contained_file


def native_names(path):
    """Use the actual Windows final-path API, never a Windows path on Unix."""
    if os.name == "nt":
        import nt
        return path, Path(nt._getfinalpathname(str(path)))
    return (path,)


class NativeContainmentContracts(unittest.TestCase):
    def assert_file(self, candidate, directory, expected):
        admitted = contained_file(candidate, directory)
        self.assertEqual(admitted, candidate.resolve(strict=True))
        self.assertTrue(admitted.samefile(expected))
        self.assertEqual(admitted.read_bytes(), b"abc")
        # NIST's SHA-256 short-message vector binds the admitted real bytes.
        self.assertEqual(hashlib.sha256(admitted.read_bytes()).hexdigest(),
                         "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")

    def link_directory(self, link, target):
        if os.name == "nt":
            shell = os.environ.get("COMSPEC") or shutil.which("cmd.exe")
            self.assertIsNotNone(shell, "The real Windows junction fixture requires cmd.exe")
            result = subprocess.run([shell, "/d", "/c", "mklink", "/J", str(link), str(target)],
                                    capture_output=True, timeout=10)
            self.assertEqual(result.returncode, 0, (result.stdout, result.stderr))
        else:
            link.symlink_to(target, target_is_directory=True)
        self.assertTrue(link.samefile(target))

    def test_direct_nested_and_native_namespace_paths_admit_real_owned_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            owner = root / "prepared 患者 λ"
            nested = owner / "module inputs"
            nested.mkdir(parents=True)
            for parent in (owner, nested):
                target = parent / "arbitrary Ω record.bin"
                target.write_bytes(b"abc")
                candidates = (*native_names(target), parent / ".." / parent.name / target.name)
                for candidate in candidates:
                    for directory in native_names(owner):
                        with self.subTest(candidate=candidate, directory=directory):
                            self.assertTrue(candidate.is_absolute())
                            self.assert_file(candidate, directory, target)

    def test_relative_paths_and_wrong_native_node_kinds_reject(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            owner = root / "owned"
            owner.mkdir()
            target = owner / "record"
            target.write_bytes(b"abc")
            cases = ((Path("record"), owner, "absolute"),
                     (target, Path("owned"), "absolute"),
                     (owner, owner, "regular file"),
                     (target, target, "owned directory"))
            for candidate, directory, cause in cases:
                with self.subTest(candidate=candidate, directory=directory):
                    with self.assertRaisesRegex(ValueError, cause):
                        contained_file(candidate, directory)
            self.assertEqual(target.read_bytes(), b"abc")

    def test_equal_content_siblings_and_outside_hardlinks_do_not_prove_containment(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            owner, sibling = root / "owned", root / "owned-copy"
            owner.mkdir()
            sibling.mkdir()
            target = owner / "record"
            target.write_bytes(b"abc")
            other = sibling / target.name
            other.write_bytes(target.read_bytes())
            hardlink = root / "outside-hardlink"
            hardlink.hardlink_to(target)
            self.assertFalse(other.samefile(target))
            self.assertTrue(hardlink.samefile(target))
            for candidate in (other, hardlink):
                with self.subTest(candidate=candidate):
                    with self.assertRaisesRegex(ValueError, "outside its owned directory"):
                        contained_file(candidate, owner)
                    self.assertEqual(candidate.read_bytes(), target.read_bytes())

    def test_resolved_symlink_or_junction_escape_rejects_but_owned_alias_admits(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            owner, outside = root / "owned", root / "outside"
            owner.mkdir()
            outside.mkdir()
            owned_file, outside_file = owner / "owned-record", outside / "outside-record"
            owned_file.write_bytes(b"abc")
            outside_file.write_bytes(b"abc")
            escape, alias = owner / "redirected", root / "owned-alias"
            self.link_directory(escape, outside)
            self.link_directory(alias, owner)
            self.assertEqual((escape / outside_file.name).resolve(strict=True), outside_file)
            with self.assertRaisesRegex(ValueError, "outside its owned directory"):
                contained_file(escape / outside_file.name, owner)
            self.assert_file(alias / owned_file.name, owner, owned_file)
            self.assert_file(owned_file, alias, owned_file)
            self.assertEqual(outside_file.read_bytes(), b"abc")

    def test_missing_or_blocked_native_paths_retain_exact_os_errors_with_context(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            owner = root / "owned"
            owner.mkdir()
            target = owner / "record"
            target.write_bytes(b"abc")
            missing = root / "missing 患者"
            blocked = target / "child"
            for candidate, directory, failed, role in ((missing, owner, missing, "candidate"),
                                                         (target, missing, missing, "owned directory"),
                                                         (blocked, owner, blocked, "candidate")):
                with self.subTest(candidate=candidate, directory=directory):
                    with self.assertRaises(OSError) as observed:
                        failed.resolve(strict=True)
                    native = observed.exception
                    expected = (type(native), native.errno, native.filename,
                                getattr(native, "winerror", None), str(native))
                    with self.assertRaises(expected[0]) as rejected:
                        contained_file(candidate, directory)
                    actual = rejected.exception
                    self.assertEqual((type(actual), actual.errno, actual.filename,
                                      getattr(actual, "winerror", None), str(actual)), expected)
                    self.assertEqual(actual.__notes__, [f"native containment resolve {role}: {failed!r}"])


if __name__ == "__main__":
    unittest.main()
