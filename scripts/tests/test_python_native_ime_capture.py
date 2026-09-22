"""Value-semantic contracts for the native Windows IME capture utility."""

from __future__ import annotations

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import python_native_ime_capture as capture


class NativeImeCaptureTests(unittest.TestCase):
    def test_layout_fixtures_have_bounded_physical_sequences_and_oracles(self) -> None:
        for fixture in capture.IME_LAYOUTS.values():
            keys = capture._ascii_keys(fixture["keys"])
            self.assertGreater(len(keys), 0)
            self.assertLessEqual(len(keys), capture.MAX_KEYS)
            self.assertTrue(fixture["expected_commit"])

    def test_ascii_key_conversion_rejects_nonletters(self) -> None:
        self.assertEqual(
            capture._ascii_keys("NiHaO"),
            (ord("N"), ord("I"), ord("H"), ord("A"), ord("O")),
        )
        with self.assertRaisesRegex(ValueError, "ASCII letters"):
            capture._ascii_keys("ni1")

    def test_source_revision_and_output_scope_are_bounded(self) -> None:
        self.assertEqual(capture._validate_source_revision("c29d8a3"), "c29d8a3")
        with self.assertRaisesRegex(ValueError, "lowercase hexadecimal"):
            capture._validate_source_revision("C29D8A3")
        with self.assertRaisesRegex(ValueError, "directly inside"):
            capture._validate_outputs(
                pathlib.Path("output/native-ime"),
                pathlib.Path("output/native-ime/nested/trace.json"),
            )

    def test_unicode_fixtures_are_exact_and_bounded(self) -> None:
        self.assertEqual(capture.UNICODE_FIXTURES["combining"], "e\u0301")
        self.assertIn("\u200d", capture.UNICODE_FIXTURES["emoji"])
        self.assertIn("\u05d0", capture.UNICODE_FIXTURES["mixed-direction"])
        for text in capture.UNICODE_FIXTURES.values():
            encoded, units = capture.unicode_capture._validate_text(text)
            self.assertLessEqual(len(encoded), capture.unicode_capture.MAX_TEXT_BYTES)
            self.assertGreater(len(units), 0)


if __name__ == "__main__":
    unittest.main()
