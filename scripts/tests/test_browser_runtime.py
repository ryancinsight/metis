"""Value-semantic tests for the dependency-free browser conformance runner."""
from __future__ import annotations

import base64
import pathlib
import sys
import tempfile
import unittest
import zlib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from browser_runtime import (
    BrowserEngine,
    BrowserRuntimeError,
    StaticServer,
    Trace,
    WebDriverClient,
    _write_trace,
    _validate_bridge_url,
    run_scenario,
)


def _png() -> bytes:
    """Return a deterministic one-pixel PNG for the screenshot contract."""
    raw = b"\x00\x00\x00\x00\xff"
    compressed = zlib.compress(raw)
    chunk = lambda name, data: len(data).to_bytes(4, "big") + name + data + zlib.crc32(name + data).to_bytes(4, "big")
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", (1).to_bytes(4, "big") + (1).to_bytes(4, "big") + b"\x08\x06\x00\x00\x00") + chunk(b"IDAT", compressed) + chunk(b"IEND", b"")


class FakeDriver:
    """A protocol-shaped test driver; production runs use WebDriverClient."""

    capabilities = {"browserName": "test", "browserVersion": "1"}

    def __init__(self) -> None:
        self.closed = False
        self.stopped = False
        self.success = False
        self.pending = False
        self.values = {"weight-kg": "72.5", "target-dose": "0.5"}

    def create_session(self, browser_name: str) -> None:
        self.capabilities = {"browserName": browser_name, "browserVersion": "test"}

    def set_timeouts(self, milliseconds: int) -> None:
        self.timeout = milliseconds

    def navigate(self, url: str) -> None:
        self.url = url

    def find(self, selector: str) -> str:
        return selector.removeprefix("#")

    def clear(self, element_id: str) -> None:
        if element_id in self.values:
            self.values[element_id] = ""

    def send_keys(self, element_id: str, value: str) -> None:
        if element_id in self.values:
            self.values[element_id] = value

    def click(self, element_id: str) -> None:
        if element_id == "submit-calculation":
            self.pending = True
            self.success = True
        elif element_id == "metis-stop":
            self.stopped = True
        elif element_id == "metis-start":
            self.stopped = False
            self.success = False
            self.pending = False

    def execute(self, script: str, arguments=()):
        if "result-metrics" in script and "return document" in script:
            return "Volume rate: 0.900000 mL/hr"
        if "Object.fromEntries(ids.map" in script:
            return self.snapshot()
        return None

    def execute_async(self, script: str, arguments=()):
        return {"ok": True}

    def screenshot(self) -> bytes:
        return _png()

    def snapshot(self):
        if self.stopped:
            return {"app_text": "Metis browser host stopped.", "mounted_controls": 0, "elements": {}}
        state = "Backend result received" if self.success else "Browser controls are active."
        return {
            "app_text": "Authorized clinical form boundary",
            "mounted_controls": 12,
            "elements": {
                "metis-status": {"text": "Authorized backend session ready", "value": None, "disabled": False},
                "metis-form": {"text": "", "value": None, "disabled": False, "busy": "false"},
                "submit-calculation": {"text": "Submit", "value": None, "disabled": False},
                "result-state": {"text": state, "value": None, "disabled": False},
                "result-weight": {"text": f"{float(self.values['weight-kg']):.2f} kg", "value": None, "disabled": False},
                "result-dose": {"text": f"{float(self.values['target-dose']):.3f} mcg/kg/min", "value": None, "disabled": False},
            },
        }

    def close(self) -> None:
        self.closed = True


class BrowserRuntimeTests(unittest.TestCase):
    """The same trace keeps its value semantics across all engine names."""

    def test_engine_names_are_closed_and_webdriver_capabilities_are_stable(self):
        self.assertEqual(BrowserEngine.parse("CHROMIUM"), BrowserEngine.CHROMIUM)
        self.assertEqual(BrowserEngine.FIREFOX.webdriver_name, "firefox")
        self.assertEqual(BrowserEngine.WEBKIT.webdriver_name, "safari")
        with self.assertRaises(BrowserRuntimeError):
            BrowserEngine.parse("blink")

    def test_authorized_trace_records_two_changes_and_cleanup(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            driver = FakeDriver()
            trace = run_scenario(
                driver,
                BrowserEngine.CHROMIUM,
                "http://127.0.0.1:8080/?endpoint=ws%3A%2F%2F127.0.0.1%3A8765%2Fsocket&process=42&principal=66666666666666666666666666666666",
                "authorized",
                "0" * 40,
                pathlib.Path(directory),
                5_000,
                False,
                4_000,
            )
        self.assertTrue(driver.closed)
        self.assertEqual([action["field"] for action in trace.actions if action["action"] == "input-change"], ["weight-kg", "target-dose"])
        self.assertEqual(trace.actions[-1], {"action": "stop-remount", "stale_result": False})
        self.assertEqual(trace.cleanup["stopped_mounted_controls"], 0)
        self.assertEqual(trace.cleanup["remounted_mounted_controls"], 12)
        self.assertEqual(trace.cleanup["pending_requests"], 0)
        self.assertEqual(trace.cleanup["pending_request_observation"], "remounted metis-form aria-busy=false")
        self.assertEqual(len(trace.screenshots), 6)
        document = Trace(BrowserEngine.FIREFOX, "http://127.0.0.1/", "disconnected", "0" * 40, {}).document()
        self.assertEqual(document["schema"], 1)
        self.assertEqual(document["unsupported_native_operations"], ["native-file-dialog", "native-process-launch", "os-permission-grant"])

    def test_disconnected_trace_rejects_privileged_submit(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            driver = FakeDriver()
            original_snapshot = driver.snapshot

            def disconnected_snapshot():
                snapshot = original_snapshot()
                submit = snapshot["elements"].get("submit-calculation")
                if submit is not None:
                    submit["disabled"] = True
                return snapshot

            driver.snapshot = disconnected_snapshot
            trace = run_scenario(
                driver,
                BrowserEngine.FIREFOX,
                "http://127.0.0.1:8080/",
                "disconnected",
                "0" * 40,
                pathlib.Path(directory),
                5_000,
                False,
                4_000,
            )
        self.assertEqual(trace.actions[-1], {"action": "stop-remount", "stale_result": False})
        self.assertIn({"action": "disconnected-submit", "state": "disabled"}, trace.actions)

    def test_screenshot_payload_has_valid_png_signature(self):
        encoded = base64.b64encode(_png()).decode()
        self.assertTrue(encoded)
        self.assertEqual(base64.b64decode(encoded)[:8], b"\x89PNG\r\n\x1a\n")

    def test_driver_escapes_opaque_element_identifiers(self):
        client = WebDriverClient("http://127.0.0.1:9515", 1)
        client.session_id = "session"
        requests = []

        def record(method, path, payload):
            requests.append((method, path, payload))
            return None

        client._request = record
        client.click("opaque/id")
        self.assertEqual(requests[0][1], "/session/session/element/opaque%2Fid/click")

    def test_authorized_url_requires_a_typed_session_tuple(self):
        valid = "http://127.0.0.1:8080/?endpoint=wss%3A%2F%2F127.0.0.1%3A8765%2Fsocket&process=42&principal=" + "a" * 32
        _validate_bridge_url(valid, "authorized")
        for invalid in (
            "http://127.0.0.1:8080/?endpoint=http%3A%2F%2F127.0.0.1%3A8765&process=42&principal=" + "a" * 32,
            "http://127.0.0.1:8080/?endpoint=wss%3A%2F%2F127.0.0.1%3A8765&process=x&principal=" + "a" * 32,
            "http://127.0.0.1:8080/?endpoint=wss%3A%2F%2F127.0.0.1%3A8765&process=42&principal=short",
        ):
            with self.assertRaises(BrowserRuntimeError):
                _validate_bridge_url(invalid, "authorized")

    def test_browser_artifacts_and_static_server_stay_inside_output(self):
        with tempfile.TemporaryDirectory() as directory:
            outside = pathlib.Path(directory)
            with self.assertRaises(BrowserRuntimeError):
                StaticServer(outside)
            with self.assertRaises(BrowserRuntimeError):
                _write_trace(outside / "trace.json", {"schema": 1})


if __name__ == "__main__":
    unittest.main()
