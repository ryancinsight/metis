"""Native ABI and error-accounting laws; no simulated Win32 focus success."""

import ctypes
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_input.windows import abi, focus_observation


class NativeFocusObservation(unittest.TestCase):
    def test_gui_thread_record_uses_windows_scalar_widths_and_native_pointer_alignment(self):
        record = focus_observation.GuiThreadInfo
        pointer = ctypes.sizeof(ctypes.c_void_p)
        self.assertIn(pointer, (4, 8))
        self.assertEqual(ctypes.sizeof(abi.Rect), 16)
        self.assertEqual(record.byte_size.offset, 0)
        self.assertEqual(record.flags.offset, 4)
        handles = ("active", "focus", "capture", "menu_owner", "move_size", "caret")
        for index, name in enumerate(handles):
            self.assertEqual(getattr(record, name).offset, 8 + index * pointer)
        self.assertEqual(record.caret_rect.offset, 8 + 6 * pointer)
        self.assertEqual(ctypes.sizeof(record), 8 + 6 * pointer + 16)
        self.assertEqual(ctypes.alignment(record), ctypes.alignment(ctypes.c_void_p))
        message = abi.WindowMessage
        self.assertEqual(message.window.offset, 0)
        self.assertEqual(message.message.offset, pointer)
        self.assertEqual(message.word_parameter.offset, 2 * pointer)
        self.assertEqual(message.long_parameter.offset, 3 * pointer)
        self.assertEqual(message.time.offset, 4 * pointer)
        self.assertEqual(message.point.offset, 4 * pointer + 4)
        self.assertEqual(message.private.offset, 4 * pointer + 12)
        self.assertEqual(ctypes.sizeof(message), 4 * pointer + 16)
        self.assertEqual(ctypes.alignment(message), ctypes.alignment(ctypes.c_void_p))

    def test_native_error_fields_survive_ascii_json_without_losing_unicode(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "missing-患者-🦀"
            try:
                path.read_bytes()
            except OSError as error:
                actual = error
            else:
                self.fail("native filesystem unexpectedly reads an absent file")
            observed = focus_observation.error_value("read actual absent file", actual)
            self.assertEqual(observed["type"], type(actual).__name__)
            self.assertEqual(observed["errno"], actual.errno)
            self.assertEqual(observed["winerror"], getattr(actual, "winerror", None))
            self.assertEqual(observed["message"], str(actual))
            self.assertEqual(observed["operation"], "read actual absent file")
            encoded = json.dumps(observed, ensure_ascii=True)
            self.assertTrue(encoded.isascii())
            self.assertEqual(json.loads(encoded), observed)
            self.assertIn("患者", json.loads(encoded)["message"])

    def test_error_annotation_keeps_original_exception_and_exact_report_values(self):
        try:
            257 // (256 - 256)
        except ZeroDivisionError as error:
            primary = error
        primary.add_note("original computation failure")
        report = {"available": None, "observed_empty": "", "observed_zero": 0,
                  "context": "患者-🦀", "count": sum(range(257))}
        focus_observation.attach_report(primary, report)
        self.assertIs(primary.focus_observation, report)
        self.assertEqual(primary.__notes__[0], "original computation failure")
        prefix = "win32_focus_observation="
        self.assertTrue(primary.__notes__[1].startswith(prefix))
        self.assertTrue(primary.__notes__[1].isascii())
        self.assertEqual(json.loads(primary.__notes__[1][len(prefix):]), report)
        self.assertEqual(report["count"], 256 * 257 // 2)

    def test_actual_read_only_observer_never_replaces_the_original_failure(self):
        value = 256 * 257 // 2
        try:
            value // (value - value)
        except ZeroDivisionError as error:
            primary = error
        original = str(primary)
        ledger = {"attempts": [], "criterion": None}
        focus_observation.observe_failure(primary, None, None, ledger)
        self.assertEqual(str(primary), original)
        report = primary.focus_observation
        self.assertEqual(report["schema"], 1)
        self.assertEqual(report["activation"], ledger)
        self.assertEqual(report["timing"], "before_input_queue_detach; non_atomic")
        if sys.platform == "win32":
            self.assertGreater(report["current_thread_id"], 0)
            self.assertIn("current_focus", report["windows"])
            self.assertIn("current", report["gui_threads"])
        else:
            self.assertEqual(report["observation_error"]["operation"], "read native focus state")
            self.assertEqual(report["observation_error"]["type"], "AttributeError")
        note = next(value for value in primary.__notes__
                    if value.startswith("win32_focus_observation="))
        self.assertEqual(json.loads(note.split("=", 1)[1]), report)


if __name__ == "__main__":
    unittest.main()
