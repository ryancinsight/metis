"""Value-semantic tests for the browser image decode probe."""
from __future__ import annotations

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from browser_assets import ASSET_PATHS, ASSET_SCRIPT, capture_assets
from browser_protocol import BrowserRuntimeError
from browser_trace import BrowserEngine, Trace


def _result() -> dict:
    return {
        "ok": True,
        "assets": [
            {
                "path": ASSET_PATHS[0],
                "source": "/assets/metis-mark.svg",
                "available": True,
                "decoder": "HTMLImageElement.decode",
                "complete": True,
                "natural_width": 256,
                "natural_height": 256,
                "same_origin": True,
            },
            {
                "path": ASSET_PATHS[1],
                "source": "/assets/metis-mark.png",
                "available": True,
                "decoder": "HTMLImageElement.decode",
                "complete": True,
                "natural_width": 1254,
                "natural_height": 1254,
                "same_origin": True,
            },
        ],
        "remaining_probe_elements": 0,
    }


class StubClient:
    def __init__(self, result: dict) -> None:
        self.result = result
        self.arguments = None

    def execute_async(self, script: str, arguments: list) -> dict:
        self.script = script
        self.arguments = arguments
        return self.result


def _trace() -> Trace:
    return Trace(BrowserEngine.CHROMIUM, "http://127.0.0.1:8080/", "disconnected", "0" * 40, {})


class BrowserAssetProbeTests(unittest.TestCase):
    def test_script_uses_native_decode_same_origin_and_cleanup(self):
        for fragment in (
            "HTMLImageElement.decode",
            "new URL(path, document.baseURI)",
            "url.origin !== window.location.origin",
            "data-metis-asset-probe",
            "container.remove()",
            "remaining_probe_elements",
        ):
            self.assertIn(fragment, ASSET_SCRIPT)

    def test_valid_result_records_intrinsic_dimensions(self):
        trace = _trace()
        client = StubClient(_result())
        measurement = capture_assets(client, trace, "initial")
        self.assertEqual(measurement["label"], "initial")
        self.assertEqual([item["path"] for item in measurement["assets"]], list(ASSET_PATHS))
        self.assertEqual(measurement["assets"][0]["natural_width"], 256)
        self.assertEqual(trace.metrics["assets"][0], measurement)
        self.assertEqual(client.arguments, [list(ASSET_PATHS), 5_000])

    def test_unavailable_asset_fails_closed(self):
        result = _result()
        result["assets"][1] = {"path": ASSET_PATHS[1], "available": False, "reason": "asset URL is cross-origin"}
        with self.assertRaisesRegex(BrowserRuntimeError, "cross-origin"):
            capture_assets(StubClient(result), _trace(), "initial")

    def test_wrong_source_fails_closed(self):
        result = _result()
        result["assets"][0]["source"] = "/assets/other.svg"
        with self.assertRaisesRegex(BrowserRuntimeError, "unexpected source"):
            capture_assets(StubClient(result), _trace(), "initial")

    def test_invalid_dimensions_and_leaked_probe_fail(self):
        result = _result()
        result["assets"][0]["natural_width"] = 0
        with self.assertRaisesRegex(BrowserRuntimeError, "width"):
            capture_assets(StubClient(result), _trace(), "initial")
        result = _result()
        result["remaining_probe_elements"] = 1
        with self.assertRaisesRegex(BrowserRuntimeError, "not released"):
            capture_assets(StubClient(result), _trace(), "initial")

    def test_labels_and_timeouts_are_bounded(self):
        with self.assertRaisesRegex(BrowserRuntimeError, "label is empty"):
            capture_assets(StubClient(_result()), _trace(), "")
        with self.assertRaisesRegex(BrowserRuntimeError, "timeout"):
            capture_assets(StubClient(_result()), _trace(), "initial", timeout_ms=0)


if __name__ == "__main__":
    unittest.main()
