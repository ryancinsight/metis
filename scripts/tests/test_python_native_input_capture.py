"""Value-semantic contracts for the native Unicode input capture utility."""

from __future__ import annotations

import ctypes
import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import python_native_input_capture as capture


class NativeInputCaptureTests(unittest.TestCase):
    def test_input_structure_keeps_the_64_bit_windows_abi_width(self) -> None:
        self.assertEqual(ctypes.sizeof(capture._KeyboardInput), 24)
        self.assertEqual(ctypes.sizeof(capture._InputUnion), 32)
        self.assertEqual(ctypes.sizeof(capture._Input), 40)

    def test_text_validation_returns_utf8_size_and_utf16_units(self) -> None:
        encoded, units = capture._validate_text("東京😀")
        self.assertEqual(len(encoded), len("東京😀".encode("utf-8")))
        self.assertEqual(
            units,
            tuple(int.from_bytes("東京😀".encode("utf-16-le")[offset : offset + 2], "little")
                  for offset in range(0, len("東京😀".encode("utf-16-le")), 2)),
        )

    def test_text_validation_rejects_empty_control_and_oversized_input(self) -> None:
        with self.assertRaisesRegex(ValueError, "must not be empty"):
            capture._validate_text("")
        with self.assertRaisesRegex(ValueError, "control character"):
            capture._validate_text("patient\n")
        with self.assertRaisesRegex(ValueError, "128-byte bound"):
            capture._validate_text("界" * 43)

    def test_parser_keeps_command_arguments_and_outputs_distinct(self) -> None:
        parsed = capture._parser().parse_args(
            [
                "--command",
                "metis-app.exe",
                "--argument=--metis-native-window",
                "--argument",
                "value with spaces",
                "--text",
                "東京",
                "--initial-output",
                "before.png",
                "--output",
                "after.png",
                "--manifest",
                "trace.json",
            ]
        )
        self.assertEqual(
            parsed.command_arguments,
            ["--metis-native-window", "value with spaces"],
        )
        self.assertEqual(parsed.initial_output, pathlib.Path("before.png"))
        self.assertEqual(parsed.output, pathlib.Path("after.png"))
        self.assertEqual(parsed.manifest, pathlib.Path("trace.json"))

    def test_paths_reject_colliding_outputs_and_wrong_extensions(self) -> None:
        with self.assertRaisesRegex(ValueError, "distinct"):
            capture._validate_paths(
                pathlib.Path(sys.executable),
                pathlib.Path("capture.png"),
                pathlib.Path("capture.png"),
                pathlib.Path("trace.json"),
            )
        with self.assertRaisesRegex(ValueError, r"\.bmp or \.png"):
            capture._validate_paths(
                pathlib.Path(sys.executable),
                pathlib.Path("before.gif"),
                pathlib.Path("after.png"),
                pathlib.Path("trace.json"),
            )


if __name__ == "__main__":
    unittest.main()
