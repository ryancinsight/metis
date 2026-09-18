"""Value-semantic tests for the bounded browser font loading probe."""
from __future__ import annotations

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

from browser_font import (
    FONT_FAMILY,
    FONT_FIXTURE_PATH,
    FONT_FIXTURE_SOURCE,
    FONT_INVALID_PATH,
    FONT_INVALID_SOURCE,
    FONT_METRIC_SIZE_PIXELS,
    FONT_SAMPLE_TEXT,
    capture_font,
)
from browser_protocol import BrowserRuntimeError


def _result() -> dict:
    """Return a successful load, apply, release and rejection observation."""
    return {
        "ok": True,
        "font": {
            "family": FONT_FAMILY,
            "sample": FONT_SAMPLE_TEXT,
            "path": FONT_FIXTURE_PATH,
            "source": FONT_FIXTURE_SOURCE,
            "status": "loaded",
            "baseline_size": 0,
            "size_before_teardown": 1,
            "size_after_teardown": 0,
            "registered_after_delete": False,
            "fallback_width": 148.0,
            "loaded_width": 307.2,
            "check_loaded": True,
            "denied_path": FONT_INVALID_PATH,
            "denied_source": FONT_INVALID_SOURCE,
            "denied_reason": "Failed to decode downloaded font",
            "denied_registered": False,
        },
    }


class StubClient:
    """Return a controlled browser observation for validator tests."""

    def __init__(self, value: dict) -> None:
        self.value = value
        self.arguments = None

    def execute_async(self, script: str, arguments):
        del script
        self.arguments = arguments
        return self.value


class Trace:
    """Carry the metric collection used by the production trace."""

    def __init__(self) -> None:
        self.metrics = {}


class BrowserFontTests(unittest.TestCase):
    def test_capture_records_loaded_face_and_release(self):
        trace = Trace()
        client = StubClient(_result())
        measurement = capture_font(client, trace, "initial")
        self.assertEqual(measurement["family"], FONT_FAMILY)
        self.assertEqual(measurement["source"], FONT_FIXTURE_SOURCE)
        self.assertEqual(measurement["status"], "loaded")
        self.assertEqual(measurement["size_after_teardown"], measurement["baseline_size"])
        self.assertGreater(measurement["loaded_width"], measurement["fallback_width"])
        self.assertEqual(trace.metrics["font"][0], measurement)
        self.assertEqual(
            client.arguments,
            [FONT_FIXTURE_PATH, FONT_INVALID_PATH, FONT_FAMILY, FONT_SAMPLE_TEXT, FONT_METRIC_SIZE_PIXELS, 5_000],
        )

    def test_capture_rejects_probe_failure(self):
        with self.assertRaisesRegex(BrowserRuntimeError, "font probe failed"):
            capture_font(StubClient({"ok": False, "error": "FontFaceSet API unavailable"}), Trace(), "initial")
        with self.assertRaisesRegex(BrowserRuntimeError, "malformed record"):
            capture_font(StubClient({"ok": True}), Trace(), "initial")

    def test_capture_rejects_unapplied_face(self):
        result = _result()
        result["font"]["loaded_width"] = result["font"]["fallback_width"]
        with self.assertRaisesRegex(BrowserRuntimeError, "separate the loaded face"):
            capture_font(StubClient(result), Trace(), "initial")
        result = _result()
        result["font"]["check_loaded"] = False
        with self.assertRaisesRegex(BrowserRuntimeError, "check did not report"):
            capture_font(StubClient(result), Trace(), "initial")
        result = _result()
        result["font"]["status"] = "unloaded"
        with self.assertRaisesRegex(BrowserRuntimeError, "did not reach the loaded state"):
            capture_font(StubClient(result), Trace(), "initial")

    def test_capture_rejects_unreleased_face(self):
        result = _result()
        result["font"]["size_before_teardown"] = 2
        with self.assertRaisesRegex(BrowserRuntimeError, "exactly one registered face"):
            capture_font(StubClient(result), Trace(), "initial")
        result = _result()
        result["font"]["size_after_teardown"] = 1
        with self.assertRaisesRegex(BrowserRuntimeError, "did not release its registered face"):
            capture_font(StubClient(result), Trace(), "initial")
        result = _result()
        result["font"]["registered_after_delete"] = True
        with self.assertRaisesRegex(BrowserRuntimeError, "left its face registered"):
            capture_font(StubClient(result), Trace(), "initial")

    def test_capture_rejects_accepted_invalid_resource(self):
        result = _result()
        result["font"]["denied_reason"] = None
        with self.assertRaisesRegex(BrowserRuntimeError, "denied reason is missing"):
            capture_font(StubClient(result), Trace(), "initial")
        result = _result()
        result["font"]["denied_reason"] = ""
        with self.assertRaisesRegex(BrowserRuntimeError, "not rejected with a reason"):
            capture_font(StubClient(result), Trace(), "initial")
        result = _result()
        result["font"]["denied_registered"] = True
        with self.assertRaisesRegex(BrowserRuntimeError, "registered as a loaded face"):
            capture_font(StubClient(result), Trace(), "initial")

    def test_capture_rejects_unexpected_identity_and_sources(self):
        for field, value, message in (
            ("family", "Comic Sans", "unexpected family"),
            ("sample", "ZZ", "unexpected sample"),
            ("path", "assets/other.woff2", "unexpected fixture path"),
            ("source", "/assets/other.woff2", "unexpected source"),
            ("denied_path", "assets/other.png", "unexpected invalid-resource path"),
            ("denied_source", "/assets/other.png", "invalid font resource resolved"),
        ):
            with self.subTest(field=field):
                result = _result()
                result["font"][field] = value
                with self.assertRaisesRegex(BrowserRuntimeError, message):
                    capture_font(StubClient(result), Trace(), "initial")

    def test_capture_rejects_bounded_measurements(self):
        for field in ("fallback_width", "loaded_width"):
            for value in (0, -1.0, float("inf"), float("nan"), 8_192.0):
                with self.subTest(field=field, value=value):
                    result = _result()
                    result["font"][field] = value
                    with self.assertRaisesRegex(BrowserRuntimeError, "width"):
                        capture_font(StubClient(result), Trace(), "initial")

    def test_capture_rejects_invalid_label_and_timeout(self):
        with self.assertRaisesRegex(BrowserRuntimeError, "label is empty"):
            capture_font(StubClient(_result()), Trace(), "")
        with self.assertRaisesRegex(BrowserRuntimeError, "timeout"):
            capture_font(StubClient(_result()), Trace(), "initial", timeout_ms=0)
        with self.assertRaisesRegex(BrowserRuntimeError, "timeout"):
            capture_font(StubClient(_result()), Trace(), "initial", timeout_ms=10_001)


if __name__ == "__main__":
    unittest.main()
