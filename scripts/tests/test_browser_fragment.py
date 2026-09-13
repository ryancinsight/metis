from __future__ import annotations

import pathlib
import tempfile
import unittest
import zlib

import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

from browser_fragment import (  # noqa: E402
    FRAGMENT_NEGATIVE,
    FRAGMENT_SUCCESS,
    run_fragment_scenario,
)
from browser_trace import BrowserEngine  # noqa: E402


def _png() -> bytes:
    raw = b"\x00\x00\x00\x00\xff"
    compressed = zlib.compress(raw)

    def chunk(name: bytes, data: bytes) -> bytes:
        return (
            len(data).to_bytes(4, "big")
            + name
            + data
            + zlib.crc32(name + data).to_bytes(4, "big")
        )

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", (1).to_bytes(4, "big") + (1).to_bytes(4, "big") + b"\x08\x06\x00\x00\x00")
        + chunk(b"IDAT", compressed)
        + chunk(b"IEND", b"")
    )


class FragmentDriver:
    """Protocol-shaped driver for the authenticated fragment trace."""

    def __init__(self) -> None:
        self.capabilities = {"browserName": "test", "browserVersion": "1"}
        self.generation = 1
        self.ready = True
        self.negative_probes = True
        self.closed = False

    def create_session(self, browser_name: str) -> None:
        self.capabilities = {"browserName": browser_name, "browserVersion": "test"}

    def set_timeouts(self, milliseconds: int) -> None:
        self.timeout = milliseconds

    def navigate(self, url: str) -> None:
        self.url = url

    def find(self, selector: str) -> str:
        return selector.removeprefix("#")

    def click(self, element_id: str) -> None:
        if element_id == "metis-reset":
            self.generation += 1
            self.ready = False
            self.negative_probes = False
        elif element_id == "metis-fragment":
            self.ready = True

    def execute(self, script: str, arguments=()):
        if "performance.memory" in script:
            return {
                "available": True,
                "source": "performance.memory",
                "used_js_heap_bytes": 100,
                "total_js_heap_bytes": 200,
                "js_heap_limit_bytes": 400,
            }
        if "Object.fromEntries(ids.map" not in script:
            return None
        negative = FRAGMENT_NEGATIVE if self.negative_probes else "—"
        status = FRAGMENT_SUCCESS if self.ready else "Mount reset; the previous fragment generation is stale"
        events = (
            "Fragment action status.describe accepted: session" if self.ready else "—"
        )
        response = (
            "200 metis-http-ready · handshake 200 · fragment 200 (1 patch)"
            if self.ready
            else "—"
        )
        return {
            "app_text": "Metis HTTP boundary",
            "elements": {
                "metis-status": {"text": status, "value": None, "disabled": False, "busy": None},
                "fragment-input": {"text": "", "value": "session", "disabled": False, "busy": None},
                "metis-fragment": {"text": "Run authenticated fragment", "value": None, "disabled": not self.ready, "busy": None},
                "metis-reset": {"text": "Reset mount", "value": None, "disabled": False, "busy": None},
                "http-response": {"text": response, "value": None, "disabled": False, "busy": None},
                "metis-events": {"text": events, "value": None, "disabled": False, "busy": None},
                "metis-negative": {"text": negative, "value": None, "disabled": False, "busy": None},
                "metis-lifecycle": {"text": f"generation {self.generation}", "value": None, "disabled": False, "busy": None},
            },
        }

    def execute_async(self, script: str, arguments=()):
        selector, expected, include, _timeout = arguments
        state = self.execute("Object.fromEntries(ids.map")
        element_id = selector.removeprefix("#")
        text = state["elements"].get(element_id, {}).get("text", "")
        return {"ok": expected in text if include else expected == text}

    def screenshot(self) -> bytes:
        return _png()

    def close(self) -> None:
        self.closed = True


class BrowserFragmentTests(unittest.TestCase):
    def test_authenticated_fragment_trace_records_success_rejection_stale_and_remount(self):
        root = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=root) as directory:
            driver = FragmentDriver()
            trace = run_fragment_scenario(
                driver,
                BrowserEngine.CHROMIUM,
                "http://127.0.0.1:8080/http-health.html",
                "0" * 40,
                pathlib.Path(directory),
                5_000,
                browser_heap=True,
            )
        self.assertTrue(driver.closed)
        self.assertEqual(trace.bridge, "http-fragment")
        self.assertEqual([item["action"] for item in trace.actions], [
            "health",
            "authenticated-fragment",
            "negative-probes",
            "reset",
            "remounted-fragment",
        ])
        self.assertEqual([item["generation"] for item in trace.snapshots], [1, 2, 2])
        self.assertEqual(
            [snapshot["elements"]["metis-negative"]["text"] for snapshot in trace.snapshots],
            [FRAGMENT_NEGATIVE, "—", "—"],
        )
        self.assertEqual(len(trace.screenshots), 3)
        self.assertEqual(len(trace.metrics["browser_heap"]), 2)
        self.assertTrue(trace.cleanup["session_closed"])

    def test_fragment_trace_rejects_missing_success_state(self):
        driver = FragmentDriver()
        driver.ready = False
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(RuntimeError, "did not contain"):
                run_fragment_scenario(
                    driver,
                    BrowserEngine.CHROMIUM,
                    "http://127.0.0.1:8080/http-health.html",
                    "0" * 40,
                    pathlib.Path(directory),
                    5_000,
                )


if __name__ == "__main__":
    unittest.main()
