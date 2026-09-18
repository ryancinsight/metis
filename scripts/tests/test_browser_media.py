"""Value-semantic tests for the browser media lifecycle probe."""
from __future__ import annotations

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

from browser_media import MEDIA_EMPTY_NETWORK_STATES, MEDIA_PROBES, capture_media
from browser_protocol import BrowserRuntimeError


def _result() -> dict:
    """Return a successful audio/video error and teardown observation."""
    return {
        "ok": True,
        "media": [
            {
                "kind": "audio",
                "source": "data:audio/wav;base64,AAAA",
                "event": "error",
                "error_code": 4,
                "error_message": None,
                "teardown": {"ready_state": 0, "network_state": 3, "current_src": "", "src_attribute": None},
                "attached_after_remove": False,
            },
            {
                "kind": "video",
                "source": "data:video/mp4;base64,AAAA",
                "event": "error",
                "error_code": 4,
                "error_message": None,
                "teardown": {"ready_state": 0, "network_state": 3, "current_src": "", "src_attribute": None},
                "attached_after_remove": False,
            },
        ],
        "remaining_probe_elements": 0,
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


class BrowserMediaTests(unittest.TestCase):
    def test_capture_records_error_and_empty_teardown(self):
        trace = Trace()
        client = StubClient(_result())
        measurement = capture_media(client, trace, "initial")
        self.assertEqual([item["kind"] for item in measurement["media"]], list(MEDIA_PROBES))
        self.assertIn(measurement["media"][0]["teardown"]["network_state"], MEDIA_EMPTY_NETWORK_STATES)
        self.assertEqual(trace.metrics["media"][0], measurement)
        self.assertEqual(client.arguments, [5_000])

    def test_capture_rejects_successful_decode(self):
        result = _result()
        result["media"][1]["event"] = "loadedmetadata"
        with self.assertRaisesRegex(BrowserRuntimeError, "did not report a decode error"):
            capture_media(StubClient(result), Trace(), "initial")

    def test_capture_rejects_unexpected_source(self):
        result = _result()
        result["media"][0]["source"] = "data:audio/wav;base64,BBBB"
        with self.assertRaisesRegex(BrowserRuntimeError, "unexpected source"):
            capture_media(StubClient(result), Trace(), "initial")

    def test_capture_rejects_retained_source_and_leaked_probe(self):
        result = _result()
        result["media"][0]["teardown"]["src_attribute"] = "data:audio/wav;base64,AAAA"
        with self.assertRaisesRegex(BrowserRuntimeError, "retained its source"):
            capture_media(StubClient(result), Trace(), "initial")
        result = _result()
        result["remaining_probe_elements"] = 1
        with self.assertRaisesRegex(BrowserRuntimeError, "not released"):
            capture_media(StubClient(result), Trace(), "initial")

    def test_capture_rejects_invalid_code_and_bounds(self):
        result = _result()
        result["media"][0]["error_code"] = 0
        with self.assertRaisesRegex(BrowserRuntimeError, "invalid MediaError code"):
            capture_media(StubClient(result), Trace(), "initial")
        with self.assertRaisesRegex(BrowserRuntimeError, "label is empty"):
            capture_media(StubClient(_result()), Trace(), "")
        with self.assertRaisesRegex(BrowserRuntimeError, "timeout"):
            capture_media(StubClient(_result()), Trace(), "initial", timeout_ms=0)


if __name__ == "__main__":
    unittest.main()
