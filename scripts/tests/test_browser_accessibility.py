"""Value-semantic tests for the browser accessibility runtime probe."""
from __future__ import annotations

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

from browser_accessibility import capture_accessibility
from browser_protocol import BrowserRuntimeError


def _snapshot() -> dict:
    """Return a bounded browser observation with one focusable control."""
    return {
        "ok": True,
        "media": {"reduced_motion": False, "forced_colors": False, "contrast_more": True},
        "viewport": {"width": 1280, "height": 720, "device_pixel_ratio": 1.25},
        "visual_viewport": {"scale": 1.0, "width": 1280.0, "height": 720.0},
        "document": {
            "client_width": 1280,
            "scroll_width": 1280,
            "client_height": 720,
            "scroll_height": 720,
        },
        "active_before": None,
        "active_after": None,
        "focus_order": [{"id": "save", "role": "button", "name": "Save"}],
        "focus_sequence": ["save"],
        "geometry": {
            "metis-app": {
                "left": 0.0,
                "top": 0.0,
                "width": 960.0,
                "height": 720.0,
                "right": 960.0,
                "bottom": 720.0,
            },
            "save": {
                "left": 16.0,
                "top": 16.0,
                "width": 120.0,
                "height": 44.0,
                "right": 136.0,
                "bottom": 60.0,
            },
        },
        "semantics": [
            {"id": "metis-app", "role": "main", "name": "metis-app", "states": {"disabled": False, "open": None, "aria_busy": None, "aria_live": "polite", "aria_atomic": None, "aria_expanded": None, "aria_haspopup": None}},
            {"id": "metis-form", "role": "form", "name": "metis-form", "states": {"disabled": False, "open": None, "aria_busy": "false", "aria_live": None, "aria_atomic": None, "aria_expanded": None, "aria_haspopup": None}},
            {"id": "session-dialog", "role": "dialog", "name": "Session details", "states": {"disabled": False, "open": False, "aria_busy": None, "aria_live": None, "aria_atomic": None, "aria_expanded": None, "aria_haspopup": None}},
            {"id": "submit-calculation", "role": "button", "name": "Submit", "states": {"disabled": False, "open": None, "aria_busy": None, "aria_live": None, "aria_atomic": None, "aria_expanded": None, "aria_haspopup": None}},
            {"id": "file-input", "role": "input", "name": "Files", "states": {"disabled": False, "open": None, "aria_busy": None, "aria_live": None, "aria_atomic": None, "aria_expanded": None, "aria_haspopup": None}},
            {"id": "text-specimen", "role": "textarea", "name": "Clinical note", "states": {"disabled": False, "open": None, "aria_busy": None, "aria_live": None, "aria_atomic": None, "aria_expanded": None, "aria_haspopup": None}},
            {"id": "explorer-table", "role": "table", "name": "Result explorer", "states": {"disabled": False, "open": None, "aria_busy": "false", "aria_live": None, "aria_atomic": None, "aria_expanded": None, "aria_haspopup": None}},
            {"id": "save", "role": "button", "name": "Save", "states": {"disabled": False, "open": None, "aria_busy": None, "aria_live": None, "aria_atomic": None, "aria_expanded": None, "aria_haspopup": None}},
        ],
    }


class StubClient:
    """Return a controlled browser observation for validator tests."""

    def __init__(self, value: dict) -> None:
        self.value = value

    def execute(self, script: str):
        del script
        return self.value


class Trace:
    """Carry the metric collection used by the production trace."""

    def __init__(self) -> None:
        self.metrics = {}


class BrowserAccessibilityTests(unittest.TestCase):
    def test_capture_records_media_focus_and_zoom_geometry(self):
        trace = Trace()
        snapshot = capture_accessibility(StubClient(_snapshot()), trace, "initial")
        self.assertEqual(snapshot["focus_order"], [{"id": "save", "role": "button", "name": "Save"}])
        self.assertEqual(snapshot["viewport"]["device_pixel_ratio"], 1.25)
        self.assertTrue(snapshot["media"]["contrast_more"])
        self.assertEqual(trace.metrics["accessibility"][0]["label"], "initial")
        self.assertEqual(snapshot["semantics"][1]["role"], "form")

    def test_capture_rejects_horizontal_overflow(self):
        value = _snapshot()
        value["document"]["scroll_width"] = 1282
        with self.assertRaisesRegex(BrowserRuntimeError, "horizontal overflow"):
            capture_accessibility(StubClient(value), Trace(), "overflow")

    def test_capture_rejects_focus_order_divergence_and_active_focus(self):
        value = _snapshot()
        value["focus_sequence"] = ["other"]
        with self.assertRaisesRegex(BrowserRuntimeError, "focus sequence"):
            capture_accessibility(StubClient(value), Trace(), "wrong-order")
        value = _snapshot()
        value["active_after"] = "save"
        with self.assertRaisesRegex(BrowserRuntimeError, "left focus active"):
            capture_accessibility(StubClient(value), Trace(), "focus-stuck")

    def test_capture_enforces_requested_media_and_baseline_order(self):
        with self.assertRaisesRegex(BrowserRuntimeError, "prefers-reduced-motion"):
            capture_accessibility(
                StubClient(_snapshot()),
                Trace(),
                "reduced-motion",
                require_reduced_motion=True,
            )
        baseline = capture_accessibility(StubClient(_snapshot()), Trace(), "baseline")
        changed = _snapshot()
        changed["focus_order"] = [{"id": "cancel", "role": "button", "name": "Cancel"}]
        changed["focus_sequence"] = ["cancel"]
        changed["geometry"] = {
            "metis-app": changed["geometry"]["metis-app"],
            "cancel": changed["geometry"]["save"],
        }
        with self.assertRaisesRegex(BrowserRuntimeError, "focus order changed"):
            capture_accessibility(StubClient(changed), Trace(), "changed", baseline=baseline)

    def test_capture_rejects_semantic_identity_change(self):
        baseline = capture_accessibility(StubClient(_snapshot()), Trace(), "baseline")
        changed = _snapshot()
        changed["semantics"][-1]["id"] = "changed-save"
        with self.assertRaisesRegex(BrowserRuntimeError, "semantic identity"):
            capture_accessibility(StubClient(changed), Trace(), "changed", baseline=baseline)

    def test_capture_rejects_missing_required_semantic_role(self):
        value = _snapshot()
        value["semantics"] = [item for item in value["semantics"] if item["id"] != "explorer-table"]
        with self.assertRaisesRegex(BrowserRuntimeError, "explorer-table"):
            capture_accessibility(StubClient(value), Trace(), "missing-semantic")


if __name__ == "__main__":
    unittest.main()
