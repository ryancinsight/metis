"""Check the browser input-to-frame latency instrument."""
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
    """Deterministic WebDriver double exposing the real probe contract."""

    def __init__(self, module, samples):
        self.module = module
        self.samples = list(samples)
        self.clicks = 0
        self.closed = False
        self.session_id = None
        self.capabilities = {
            "browserName": "chrome",
            "browserVersion": "test",
            "platformName": "test",
        }

    def create_session(self, browser_name, device_scale_milli=None, *, headless=False):
        self.session_id = "session"

    def set_timeouts(self, milliseconds):
        self.timeout = milliseconds

    def set_window_rect(self, width, height):
        self.window = (width, height)

    def navigate(self, url):
        self.url = url

    def find(self, selector):
        if selector != "#submit-calculation":
            raise AssertionError(selector)
        return "element"

    def click(self, element):
        if element != "element":
            raise AssertionError(element)
        self.clicks += 1

    def execute(self, script, arguments=()):
        if script == self.module["INSTALL_SCRIPT"]:
            return {"ok": True, "listener_count": 1}
        if script == self.module["CLEANUP_SCRIPT"]:
            return {"ok": True, "listener_count": 1}
        if arguments:
            raise AssertionError((script, arguments))
        return {
            "device_pixel_ratio": 1,
            "inner_width": 1440,
            "inner_height": 1100,
        }

    def execute_async(self, script, arguments=()):
        if script != self.module["WAIT_SCRIPT"]:
            raise AssertionError(script)
        if self.clicks > len(self.samples):
            raise AssertionError(self.clicks)
        return {"ok": True, "events": [self.samples[self.clicks - 1]]}

    def close(self):
        self.closed = True
        self.session_id = None


class InputLatencyContractTests(unittest.TestCase):
    """Exercise bounded validation and statistical value semantics."""

    @classmethod
    def setUpClass(cls):
        cls.module = runpy.run_path(str(SCRIPTS / "browser_input_latency.py"))

    def _sample(self, event, frame, *, trusted=True):
        return {
            "type": "click",
            "is_trusted": trusted,
            "target_id": "submit-calculation",
            "matches_selector": True,
            "event_ms": event,
            "frame_ms": frame,
            "latency_ms": frame - event,
        }

    def test_measurement_records_each_trusted_click_and_summary(self):
        samples = [self._sample(10, 12), self._sample(20, 24), self._sample(30, 36)]
        client = _FakeClient(self.module, samples)
        trace = self.module["Trace"](
            self.module["BrowserEngine"].CHROMIUM,
            "http://127.0.0.1:8000/app",
            "input-latency",
            "a" * 40,
            client.capabilities,
        )
        result = self.module["measure_input_latency"](
            client,
            trace,
            "#submit-calculation",
            samples=3,
            timeout_ms=100,
        )
        self.assertEqual(client.clicks, 3)
        self.assertEqual(result["sample_count"], 3)
        self.assertEqual(result["latencies_ms"], [2.0, 4.0, 6.0])
        self.assertEqual(result["mean_ms"], 4.0)
        self.assertAlmostEqual(result["stddev_ms"], 1.632993161855452)
        self.assertEqual(len(trace.actions), 3)
        self.assertEqual(trace.actions[0]["action"], "trusted-click")
        self.assertEqual(trace.metrics["input_to_frame_latency"], result)

    def test_untrusted_event_is_rejected(self):
        sample = self._sample(10, 12, trusted=False)
        client = _FakeClient(self.module, [sample, sample])
        trace = self.module["Trace"](
            self.module["BrowserEngine"].CHROMIUM,
            "http://127.0.0.1:8000/app",
            "input-latency",
            "a" * 40,
            client.capabilities,
        )
        with self.assertRaisesRegex(self.module["BrowserRuntimeError"], "trusted click"):
            self.module["measure_input_latency"](
                client, trace, "#submit-calculation", samples=2, timeout_ms=100
            )

    def test_timestamp_mismatch_is_rejected(self):
        sample = self._sample(10, 12)
        sample["latency_ms"] = 7
        client = _FakeClient(self.module, [sample, sample])
        trace = self.module["Trace"](
            self.module["BrowserEngine"].CHROMIUM,
            "http://127.0.0.1:8000/app",
            "input-latency",
            "a" * 40,
            client.capabilities,
        )
        with self.assertRaisesRegex(self.module["BrowserRuntimeError"], "inconsistent"):
            self.module["measure_input_latency"](
                client, trace, "#submit-calculation", samples=2, timeout_ms=100
            )

    def test_bounds_reject_invalid_selector_samples_and_budget(self):
        validate_selector = self.module["validate_selector"]
        validate_samples = self.module["validate_samples"]
        validate_timeout = self.module["validate_timeout"]
        error = self.module["BrowserRuntimeError"]
        with self.assertRaisesRegex(error, "non-empty"):
            validate_selector("  ")
        with self.assertRaisesRegex(error, "control"):
            validate_selector("#but\nton")
        with self.assertRaisesRegex(error, "between 2"):
            validate_samples(1)
        with self.assertRaisesRegex(error, "between 1"):
            validate_timeout(0)
        with self.assertRaisesRegex(error, "five minutes"):
            self.module["measure_input_latency"](
                _FakeClient(self.module, [self._sample(1, 2)] * 2),
                self.module["Trace"](
                    self.module["BrowserEngine"].CHROMIUM,
                    "http://127.0.0.1:8000/app",
                    "input-latency",
                    "a" * 40,
                    {},
                ),
                "#submit-calculation",
                samples=3,
                timeout_ms=120_000,
            )

    def test_run_writes_closed_success_trace(self):
        module = self.module
        samples = [self._sample(10, 12), self._sample(20, 24)]

        def make_client(endpoint, timeout):
            return _FakeClient(module, samples)

        with tempfile.TemporaryDirectory(
            prefix="metis-input-latency-", dir=module["ROOT"] / "output"
        ) as directory:
            output = pathlib.Path(directory) / "trace.json"
            run_globals = module["run"].__globals__
            original_client = run_globals["WebDriverClient"]
            run_globals["WebDriverClient"] = make_client
            try:
                with mock.patch.object(
                    module["subprocess"],
                    "check_output",
                    return_value="b" * 40,
                ):
                    result = module["run"](
                        module["argparse"].Namespace(
                            driver_url="http://127.0.0.1:9515",
                            url="http://127.0.0.1:8000/app",
                            engine="chromium",
                            browser_name=None,
                            device_scale=None,
                            selector="#submit-calculation",
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
            self.assertEqual(document["metrics"]["input_to_frame_latency"]["sample_count"], 2)


if __name__ == "__main__":
    unittest.main()
