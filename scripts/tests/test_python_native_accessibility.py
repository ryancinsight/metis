"""Value-semantic contracts for the Windows UI Automation journey."""

from __future__ import annotations

import pathlib
import subprocess
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import python_native_accessibility as capture


class NativeAccessibilityTests(unittest.TestCase):
    def test_value_validation_rejects_empty_controls_and_overflow(self) -> None:
        with self.assertRaisesRegex(ValueError, "must not be empty"):
            capture._validate_value("")
        with self.assertRaisesRegex(ValueError, "control character"):
            capture._validate_value("patient\n")
        with self.assertRaisesRegex(ValueError, "128-byte bound"):
            capture._validate_value("界" * 43)

    def test_parser_preserves_arguments_and_distinct_outputs(self) -> None:
        parsed = capture._parser().parse_args(
            [
                "--command",
                "metis-app.exe",
                "--argument=--metis-native-window",
                "--argument",
                "60",
                "--surface",
                "webview2",
                "--patient-value",
                "UIA-PATIENT",
                "--initial-output",
                "before.png",
                "--output",
                "after.png",
                "--manifest",
                "trace.json",
            ]
        )
        self.assertEqual(parsed.command_arguments, ["--metis-native-window", "60"])
        self.assertEqual(parsed.surface, "webview2")
        self.assertEqual(parsed.patient_value, "UIA-PATIENT")
        self.assertEqual(parsed.initial_output, pathlib.Path("before.png"))
        self.assertEqual(parsed.output, pathlib.Path("after.png"))
        self.assertEqual(parsed.manifest, pathlib.Path("trace.json"))

    def test_script_contains_real_uia_actions_and_bounded_tree(self) -> None:
        script = capture._powershell_script(42, "UIA-PATIENT")
        self.assertIn("AutomationElement]::FromHandle", script)
        self.assertIn("ValuePattern]::Pattern", script)
        self.assertIn("ValuePattern.SetValue", script)
        self.assertIn("InvokePattern]::Pattern", script)
        self.assertIn("InvokePattern.Invoke", script)
        self.assertIn("records.Count -gt 256", script)

    def test_webview2_script_targets_real_dom_controls_and_live_status(self) -> None:
        script = capture._powershell_script(42, "UIA-PATIENT", "webview2")
        self.assertIn("AutomationId -eq 'patient-id'", script)
        self.assertIn("Name -eq 'Submit calculation'", script)
        self.assertIn("AutomationId -eq 'result'", script)
        self.assertIn("ControlType.Text", script)
        self.assertIn("surface = $surface", script)

    def test_surface_preparation_primes_only_webview2_focus(self) -> None:
        with patch.object(capture.native_input, "_focus_window") as focus:
            with patch.object(capture.native_input, "_send_virtual_key") as key:
                with patch.object(capture.native_input, "_flush_window") as flush:
                    self.assertEqual(capture._prepare_surface(42, "native"), 0)
                    focus.assert_not_called()
                    self.assertEqual(
                        capture._prepare_surface(42, "webview2"),
                        capture.WEBVIEW2_FOCUS_PRIMER_TABS,
                    )
        focus.assert_called_once_with(42)
        self.assertEqual(key.call_count, 4)
        self.assertEqual(flush.call_count, 2)

    def test_powershell_trace_parser_rejects_non_object_schema(self) -> None:
        completed = subprocess.CompletedProcess(
            args=["powershell.exe"], returncode=0, stdout="[]\n", stderr=""
        )
        with patch.object(capture.subprocess, "run", return_value=completed):
            with self.assertRaisesRegex(RuntimeError, "unsupported schema"):
                capture._run_uia(42, "UIA-PATIENT")

    def test_powershell_trace_parser_preserves_value_and_status(self) -> None:
        completed = subprocess.CompletedProcess(
            args=["powershell.exe"],
            returncode=0,
            stdout='{"schema":1,"value":{"after":"UIA-PATIENT"},"status":"Rate: 1"}\n',
            stderr="",
        )
        with patch.object(capture.subprocess, "run", return_value=completed):
            result = capture._run_uia(42, "UIA-PATIENT")
        self.assertEqual(result["value"]["after"], "UIA-PATIENT")
        self.assertEqual(result["status"], "Rate: 1")


if __name__ == "__main__":
    unittest.main()
