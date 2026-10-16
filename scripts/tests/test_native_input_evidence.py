"""Exact computed capture evidence, without invoking native input delivery."""
from __future__ import annotations

import ast
import hashlib
import itertools
import pathlib
import re
import sys
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
from native_input import evidence
from native_input.windows import keyboard


class NativeInputEvidenceTests(unittest.TestCase):
    def test_each_input_combination_retains_exact_mechanism_and_unicode_values(self):
        encoded, units = keyboard.validate_text("東京😀")
        expected = {
            (False, False, False): "",
            (False, False, True): "Win32 SendInput MOUSEEVENTF_ABSOLUTE",
            (False, True, False): "Win32 SendInput KEYEVENTF_SCANCODE",
            (False, True, True): "Win32 SendInput KEYEVENTF_SCANCODE + Win32 SendInput MOUSEEVENTF_ABSOLUTE",
            (True, False, False): "Win32 SendInput KEYEVENTF_UNICODE",
            (True, False, True): "Win32 SendInput KEYEVENTF_UNICODE + Win32 SendInput MOUSEEVENTF_ABSOLUTE",
            (True, True, False): "Win32 SendInput KEYEVENTF_UNICODE + Win32 SendInput KEYEVENTF_SCANCODE",
            (True, True, True): "Win32 SendInput KEYEVENTF_UNICODE + Win32 SendInput KEYEVENTF_SCANCODE + Win32 SendInput MOUSEEVENTF_ABSOLUTE",
        }
        for text, shortcut, click in itertools.product((False, True), repeat=3):
            with self.subTest(text=text, shortcut=shortcut, click=click):
                click_record = evidence.click((512, 616), (640, 770)) if click else None
                observation = evidence.InputObservation(
                    encoded if text else b"", units if text else (), text,
                    "control-enter" if shortcut else None, 0x409,
                    {"kind": "content-child", "class": "Chrome_RenderWidgetHostHWND"},
                    True, None, click_record,
                )
                record = evidence.record(observation)
                self.assertEqual(record["input"]["mechanism"], expected[(text, shortcut, click)])
                self.assertEqual(record["input"]["utf8_bytes"], len(encoded) if text else 0)
                self.assertEqual(record["input"]["utf16_units"], len(units) if text else 0)
                self.assertEqual(record["input"]["text_sha256"], hashlib.sha256(encoded).hexdigest() if text else None)
                self.assertEqual(record["input"]["target_keyboard_layout"], "0x0000000000000409")
                self.assertEqual(record["input"]["click"], click_record)
                self.assertEqual(record["ime"], {
                    "status": "not_exercised",
                    "reason": "KEYEVENTF_UNICODE delivers committed text; an installed IME requires a separate host journey",
                })

    def test_scroll_and_click_records_keep_the_original_labels_and_observations(self):
        self.assertEqual(evidence.scroll((512, 600), -12, 5, True), {
            "mechanism": "Win32 SendInput MOUSEEVENTF_WHEEL",
            "client_point": [512, 600], "notches": -12,
            "settle_rounds": 5, "frame_changed": True,
        })
        self.assertEqual(evidence.click((512, 616), (640, 770)), {
            "mechanism": "Win32 SendInput MOUSEEVENTF_ABSOLUTE",
            "client_point": [512, 616], "screen_point": [640, 770],
        })

    def test_string_literals_do_not_qualify_windows_event_names_with_modules(self):
        qualified = []
        pattern = re.compile(r"\b(?:abi|keyboard|pointer)\.(?:KEYEVENTF|MOUSEEVENTF)_")
        for package in ("native_input", "native_capture"):
            for path in (SCRIPTS / package).rglob("*.py"):
                for node in ast.walk(ast.parse(path.read_text())):
                    if isinstance(node, ast.Constant) and isinstance(node.value, str) and pattern.search(node.value):
                        qualified.append((str(path.relative_to(SCRIPTS)), node.lineno))
        self.assertEqual(qualified, [])


if __name__ == "__main__":
    unittest.main()
