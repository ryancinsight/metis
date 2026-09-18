"""Value-semantic tests for the browser media lifecycle probe."""
from __future__ import annotations

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

from browser_media import (
    MEDIA_EMPTY_NETWORK_STATES,
    MEDIA_PLAYBACK_PATHS,
    MEDIA_PLAYBACK_REQUIRED_EVENTS,
    MEDIA_PROBES,
    capture_media,
    capture_media_playback,
)
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


def _playback_result() -> dict:
    """Return a successful same-origin audio controls observation."""
    return {
        "ok": True,
        "playback": {
            "kind": "audio",
            "path": MEDIA_PLAYBACK_PATHS[0],
            "source": "/assets/metis-tone.wav",
            "events": list(MEDIA_PLAYBACK_REQUIRED_EVENTS),
            "before": {
                "controls": True,
                "paused": True,
                "ready_state": 4,
                "network_state": 2,
                "duration_seconds": 0.2,
                "source": "/assets/metis-tone.wav",
            },
            "playing": {"paused": False, "ready_state": 4},
            "after_pause": {"paused": True, "ready_state": 4},
            "teardown": {"ready_state": 0, "network_state": 0, "current_src": "", "src_attribute": None},
            "attached_after_remove": False,
        },
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

    def find(self, selector: str):
        self.selector = selector
        return selector

    def click(self, element):
        self.clicked = element


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

    def test_playback_records_controls_and_release(self):
        trace = Trace()
        client = StubClient(_playback_result())
        measurement = capture_media_playback(client, trace, "initial")
        self.assertEqual(measurement["events"], list(MEDIA_PLAYBACK_REQUIRED_EVENTS))
        self.assertEqual(measurement["before"]["source"], "/assets/metis-tone.wav")
        self.assertTrue(measurement["before"]["controls"])
        self.assertFalse(measurement["playing"]["paused"])
        self.assertTrue(measurement["after_pause"]["paused"])
        self.assertEqual(trace.metrics["media_playback"][0], measurement)
        self.assertEqual(client.arguments, [MEDIA_PLAYBACK_PATHS[0], 5_000])

    def test_playback_rejects_wrong_events_or_source(self):
        result = _playback_result()
        result["playback"]["events"] = ["loadedmetadata"]
        with self.assertRaisesRegex(BrowserRuntimeError, "event sequence"):
            capture_media_playback(StubClient(result), Trace(), "initial")
        result = _playback_result()
        result["playback"]["source"] = "/other.wav"
        with self.assertRaisesRegex(BrowserRuntimeError, "unexpected source"):
            capture_media_playback(StubClient(result), Trace(), "initial")

    def test_playback_rejects_nonpositive_duration_and_retained_source(self):
        result = _playback_result()
        result["playback"]["before"]["duration_seconds"] = 0
        with self.assertRaisesRegex(BrowserRuntimeError, "duration"):
            capture_media_playback(StubClient(result), Trace(), "initial")
        result = _playback_result()
        result["playback"]["teardown"]["src_attribute"] = "/assets/metis-tone.wav"
        with self.assertRaisesRegex(BrowserRuntimeError, "retained its source"):
            capture_media_playback(StubClient(result), Trace(), "initial")

    def test_playback_accepts_empty_ready_state_source_reflection(self):
        result = _playback_result()
        result["playback"]["teardown"]["current_src"] = "http://127.0.0.1:12345/assets/metis-tone.wav"
        measurement = capture_media_playback(StubClient(result), Trace(), "initial")
        self.assertEqual(
            measurement["teardown"]["current_src"],
            "http://127.0.0.1:12345/assets/metis-tone.wav",
        )

    def test_playback_rejects_invalid_label_and_timeout(self):
        with self.assertRaisesRegex(BrowserRuntimeError, "label is empty"):
            capture_media_playback(StubClient(_playback_result()), Trace(), "")
        with self.assertRaisesRegex(BrowserRuntimeError, "timeout"):
            capture_media_playback(StubClient(_playback_result()), Trace(), "initial", timeout_ms=0)


if __name__ == "__main__":
    unittest.main()
