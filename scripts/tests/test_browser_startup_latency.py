"""Check the browser navigation startup latency instrument."""
from __future__ import annotations

import json
import pathlib
import runpy
import sys
import tempfile
import unittest
from unittest import mock


SCRIPTS = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))


class _FakeClient:
    """Deterministic WebDriver double exposing the startup contract."""

    def __init__(self, module, navigations, ready, frames):
        self.module = module
        self.navigations = list(navigations)
        self.ready = list(ready)
        self.frames = list(frames)
        self.navigation_calls = 0
        self.closed = False
        self.session_id = None
        self.capabilities = {
            "browserName": "chrome",
            "browserVersion": "test",
            "platformName": "test",
        }

    def create_session(self, browser_name, device_scale_milli=None, *, headless=False):
        del device_scale_milli, headless
        self.session_id = "session"
        self.capabilities["browserName"] = browser_name

    def set_timeouts(self, milliseconds):
        self.timeout = milliseconds

    def set_window_rect(self, width, height):
        self.window = (width, height)

    def navigate(self, url):
        self.url = url
        self.navigation_calls += 1

    def execute(self, script, arguments=()):
        del arguments
        if "device_pixel_ratio" in script:
            return {"device_pixel_ratio": 1, "inner_width": 1440, "inner_height": 1100}
        if script != self.module["NAVIGATION_SCRIPT"]:
            raise AssertionError(script)
        return self.navigations[self.navigation_calls - 1]

    def execute_async(self, script, arguments=()):
        del arguments
        sample = self.navigation_calls - 1
        if script == self.module["READY_SCRIPT"]:
            return self.ready[sample]
        if script == self.module["FRAME_SCRIPT"]:
            return self.frames[sample]
        raise AssertionError(script)

    def close(self):
        self.closed = True
        self.session_id = None


class StartupLatencyContractTests(unittest.TestCase):
    """Exercise startup milestone validation and bounded summaries."""

    @classmethod
    def setUpClass(cls):
        cls.module = runpy.run_path(str(SCRIPTS / "browser_startup_latency.py"))

    @staticmethod
    def _navigation(load_ms=20):
        return {
            "ok": True,
            "entry_type": "navigation",
            "start_ms": 0,
            "response_end_ms": 2,
            "dom_interactive_ms": 8,
            "dom_content_loaded_ms": 12,
            "load_event_end_ms": load_ms,
            "duration_ms": load_ms,
            "ready_state": "complete",
        }

    @staticmethod
    def _ready(value):
        return {"ok": True, "ready_ms": value, "ready_state": "complete"}

    @staticmethod
    def _frame(value):
        return {"ok": True, "frame_ms": value}

    def _trace(self, client):
        return self.module["Trace"](
            self.module["BrowserEngine"].CHROMIUM,
            "http://127.0.0.1:8000/app",
            "startup-latency",
            "a" * 40,
            client.capabilities,
        )

    def test_measurement_records_navigation_ready_and_frame_statistics(self):
        module = self.module
        client = _FakeClient(
            module,
            [self._navigation(20), self._navigation(24), self._navigation(28)],
            [self._ready(22), self._ready(30), self._ready(38)],
            [self._frame(32), self._frame(44), self._frame(56)],
        )
        trace = self._trace(client)
        result = module["measure_startup_latency"](
            client,
            trace,
            "http://127.0.0.1:8000/app",
            "#metis-form",
            samples=3,
            timeout_ms=100,
        )
        self.assertEqual(client.navigation_calls, 3)
        self.assertEqual(result["sample_count"], 3)
        self.assertEqual(result["navigation_to_ready"]["mean_ms"], 30.0)
        self.assertEqual(result["navigation_to_first_frame"]["mean_ms"], 44.0)
        self.assertEqual(result["samples"][0]["navigation"]["load_event_end_ms"], 20.0)
        self.assertEqual(trace.metrics["startup_latency"], result)
        self.assertEqual(len(trace.actions), 3)

    def test_navigation_order_and_readiness_are_value_checked(self):
        module = self.module
        broken = self._navigation()
        broken["dom_interactive_ms"] = 1
        client = _FakeClient(module, [broken, broken], [self._ready(22)] * 2, [self._frame(32)] * 2)
        with self.assertRaisesRegex(module["BrowserRuntimeError"], "not monotonic"):
            module["measure_startup_latency"](
                client, self._trace(client), "http://127.0.0.1:8000/app", "#metis-form", samples=2, timeout_ms=100
            )

        client = _FakeClient(
            module,
            [self._navigation(), self._navigation()],
            [{"ok": False, "error": "readiness selector deadline exceeded"}] * 2,
            [self._frame(32)] * 2,
        )
        with self.assertRaisesRegex(module["BrowserRuntimeError"], "readiness observation failed"):
            module["measure_startup_latency"](
                client, self._trace(client), "http://127.0.0.1:8000/app", "#metis-form", samples=2, timeout_ms=100
            )

        client = _FakeClient(
            module,
            [self._navigation(), self._navigation()],
            [self._ready(22)] * 2,
            [self._frame(21), self._frame(21)],
        )
        with self.assertRaisesRegex(module["BrowserRuntimeError"], "precedes readiness"):
            module["measure_startup_latency"](
                client, self._trace(client), "http://127.0.0.1:8000/app", "#metis-form", samples=2, timeout_ms=100
            )

    def test_bounds_reject_invalid_selector_samples_timeout_and_budget(self):
        module = self.module
        error = module["BrowserRuntimeError"]
        with self.assertRaisesRegex(error, "non-empty"):
            module["validate_selector"]("  ")
        with self.assertRaisesRegex(error, "control"):
            module["validate_selector"]("#metis\nform")
        with self.assertRaisesRegex(error, "between 2"):
            module["validate_samples"](1)
        with self.assertRaisesRegex(error, "between 1"):
            module["validate_timeout"](0)
        with self.assertRaisesRegex(error, "five minutes"):
            module["validate_budget"](8, 120_000)

    def test_run_writes_closed_success_trace(self):
        module = self.module
        navigations = [self._navigation(20), self._navigation(24)]
        ready = [self._ready(22), self._ready(30)]
        frames = [self._frame(32), self._frame(44)]

        def make_client(endpoint, timeout):
            del endpoint, timeout
            return _FakeClient(module, navigations, ready, frames)

        with tempfile.TemporaryDirectory(prefix="metis-startup-latency-", dir=module["ROOT"] / "output") as directory:
            output = pathlib.Path(directory) / "trace.json"
            run_globals = module["run"].__globals__
            original_client = run_globals["WebDriverClient"]
            run_globals["WebDriverClient"] = make_client
            try:
                with mock.patch.object(module["subprocess"], "check_output", return_value="b" * 40):
                    result = module["run"](
                        module["argparse"].Namespace(
                            driver_url="http://127.0.0.1:9515",
                            url="http://127.0.0.1:8000/app",
                            engine="chromium",
                            browser_name=None,
                            device_scale=None,
                            ready_selector="#metis-form",
                            samples=2,
                            timeout_ms=100,
                            width=1440,
                            height=1100,
                            headless=True,
                            output=output,
                        )
                    )
            finally:
                run_globals["WebDriverClient"] = original_client
            self.assertEqual(result, 0)
            document = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(document["status"], "passed")
            self.assertTrue(document["cleanup"]["session_closed"])
            self.assertEqual(document["metrics"]["startup_latency"]["sample_count"], 2)


if __name__ == "__main__":
    unittest.main()
