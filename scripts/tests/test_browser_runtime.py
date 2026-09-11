"""Value-semantic tests for the dependency-free browser conformance runner."""
from __future__ import annotations

import base64
import json
import pathlib
import sys
import tempfile
import unittest
import zlib
from unittest import mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import browser_protocol
from browser_runtime import (
    StaticServer,
    _write_trace,
    _validate_bridge_url,
    main,
    run_scenario,
)
from browser_canvas import (
    MAX_CANVAS_ATTRIBUTES,
    run_canvas_scenario,
    validate_canvas_attributes,
    validate_canvas_ids,
    validate_consumer_revision,
)
from browser_protocol import BrowserRuntimeError, MAX_SCREENSHOT_BYTES, MAX_SCREENSHOT_RESPONSE_BYTES, MAX_TRACE_BYTES, WebDriverClient
from browser_trace import BrowserEngine, Trace


def _png() -> bytes:
    """Return a deterministic one-pixel PNG for the screenshot contract."""
    raw = b"\x00\x00\x00\x00\xff"
    compressed = zlib.compress(raw)
    chunk = lambda name, data: len(data).to_bytes(4, "big") + name + data + zlib.crc32(name + data).to_bytes(4, "big")
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", (1).to_bytes(4, "big") + (1).to_bytes(4, "big") + b"\x08\x06\x00\x00\x00") + chunk(b"IDAT", compressed) + chunk(b"IEND", b"")


def _png_with_text(payload_bytes: int) -> bytes:
    """Return a valid-dimension PNG with a bounded ancillary payload."""
    image = _png()
    data = b"Comment\x00" + b"x" * payload_bytes
    chunk = len(data).to_bytes(4, "big") + b"tEXt" + data + zlib.crc32(b"tEXt" + data).to_bytes(4, "big")
    return image[:-12] + chunk + image[-12:]


class JsonResponse:
    """Small context-manager response used to exercise the production transport."""

    def __init__(self, payload: bytes) -> None:
        self.payload = payload
        self.read_limit = 0

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, traceback):
        del exc_type, exc, traceback

    def read(self, limit: int) -> bytes:
        self.read_limit = limit
        return self.payload[: limit]


class FakeDriver:
    """A protocol-shaped test driver; production runs use WebDriverClient."""

    capabilities = {"browserName": "test", "browserVersion": "1"}

    def __init__(self) -> None:
        self.session_id = None
        self.closed = False
        self.stopped = False
        self.success = False
        self.pending = False
        self.released = False
        self.canvas_actions = []
        self.values = {"weight-kg": "72.5", "target-dose": "0.5"}

    def create_session(self, browser_name: str) -> None:
        self.session_id = "session"
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
            self.success = False
        elif element_id == "metis-stop":
            self.stopped = True
            self.pending = False
            self.success = False
        elif element_id == "metis-start":
            self.stopped = False
            self.success = False
            self.pending = False

    def execute(self, script: str, arguments=()):
        if "canvas.tagName.toLowerCase()" in script:
            canvas_id = arguments[0]
            attribute_names = arguments[1]
            return {
                "id": canvas_id,
                "width": 512,
                "height": 512,
                "css_width": 512.0,
                "css_height": 512.0,
                "left": 0.0,
                "top": 0.0,
                "attributes": {
                    name: "opaque-value" if name == "data-consumer-state" else None
                    for name in attribute_names
                },
            }
        if "result-metrics" in script and "return document" in script:
            return "Volume rate: 0.900000 mL/hr" if self.success else ""
        if "Object.fromEntries(ids.map" in script:
            return self.snapshot()
        return None

    def execute_async(self, script: str, arguments=()):
        if "MutationObserver" not in script:
            return {"ok": True}
        selector, expected, include, _timeout = arguments
        if expected == "Backend result received" and self.pending and not self.stopped:
            self.success = True
            self.pending = False
        snapshot = self.snapshot()
        if selector == "#metis-app":
            text = snapshot.get("app_text", "")
        else:
            element = snapshot["elements"].get(selector.removeprefix("#"))
            if not isinstance(element, dict):
                return {"ok": False}
            text = element.get("text", "")
        matched = expected in text if include else expected == text
        return {"ok": matched}

    def screenshot(self) -> bytes:
        return _png()

    def element_screenshot(self, element_id: str) -> bytes:
        self.canvas_actions.append(("element-screenshot", element_id))
        return _png()

    def pointer_drag(self, element_id, start, end) -> None:
        self.canvas_actions.append(("pointer-drag", element_id, start, end))

    def wheel(self, element_id, position, delta) -> None:
        self.canvas_actions.append(("wheel", element_id, position, delta))

    def release_actions(self) -> None:
        self.released = True

    def snapshot(self):
        if self.stopped:
            return {"app_text": "Metis browser host stopped.", "mounted_controls": 0, "elements": {}}
        state = "Backend result received" if self.success else "Request in progress" if self.pending else "Browser controls are active."
        metrics = "Volume rate: 0.900000 mL/hr" if self.success else ""
        return {
            "app_text": "Authorized clinical form boundary",
            "mounted_controls": 12,
            "elements": {
                "metis-status": {"text": "Authorized backend session ready", "value": None, "disabled": False},
                "metis-form": {"text": "", "value": None, "disabled": False, "busy": "true" if self.pending else "false"},
                "submit-calculation": {"text": "Submit", "value": None, "disabled": False},
                "result-state": {"text": state, "value": None, "disabled": False},
                "result-metrics": {"text": metrics, "value": None, "disabled": False},
                "result-weight": {"text": f"{float(self.values['weight-kg']):.2f} kg", "value": None, "disabled": False},
                "result-dose": {"text": f"{float(self.values['target-dose']):.3f} mcg/kg/min", "value": None, "disabled": False},
            },
        }

    def close(self) -> None:
        self.session_id = None
        self.closed = True


class NoInputDriver(FakeDriver):
    """Driver mutant that ignores both input mutations."""

    def clear(self, element_id: str) -> None:
        del element_id

    def send_keys(self, element_id: str, value: str) -> None:
        del element_id, value


class InvalidAttributeDriver(FakeDriver):
    """Driver mutant that violates the opaque attribute response contract."""

    def __init__(self, attributes) -> None:
        super().__init__()
        self.attributes = attributes

    def execute(self, script: str, arguments=()):
        value = super().execute(script, arguments)
        if "canvas.tagName.toLowerCase()" in script:
            value["attributes"] = self.attributes
        return value


class RetainingDriver(FakeDriver):
    """Driver mutant that leaves controls mounted after the stop command."""

    def snapshot(self):
        snapshot = super().snapshot()
        if self.stopped:
            snapshot["mounted_controls"] = 12
        else:
            snapshot["elements"]["submit-calculation"]["disabled"] = True
        return snapshot


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

    def test_canvas_trace_records_trusted_actions_and_element_captures(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            driver = FakeDriver()
            trace = run_canvas_scenario(
                driver,
                BrowserEngine.CHROMIUM,
                "http://127.0.0.1:8080/ritk.html",
                "0" * 40,
                pathlib.Path(directory),
                5_000,
                ["ritk-snap-axial", "ritk-snap-coronal", "ritk-snap-sagittal"],
                "1" * 40,
            )
        self.assertTrue(driver.closed)
        self.assertTrue(driver.released)
        self.assertEqual(trace.bridge, "canvas")
        self.assertEqual(trace.consumer_revision, "1" * 40)
        self.assertEqual(trace.document()["consumer_revision"], "1" * 40)
        self.assertEqual(trace.cleanup["canvas_count"], 3)
        self.assertEqual(trace.cleanup["canvas_attribute_names"], [])
        self.assertEqual(
            [action["action"] for action in trace.actions],
            ["trusted-pointer-drag", "trusted-wheel"] * 3,
        )
        self.assertEqual(len(trace.snapshots), 6)
        self.assertEqual(len(trace.screenshots), 8)
        self.assertTrue(all(item["scope"] == "element" for item in trace.screenshots[1:-1]))

    def test_canvas_trace_captures_only_requested_opaque_attributes(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            trace = run_canvas_scenario(
                FakeDriver(),
                BrowserEngine.CHROMIUM,
                "http://127.0.0.1:8080/ritk.html",
                "0" * 40,
                pathlib.Path(directory),
                5_000,
                ["ritk-snap-axial"],
                canvas_attributes=("data-consumer-state", "data-consumer-missing"),
            )
        self.assertEqual(
            trace.cleanup["canvas_attribute_names"],
            ["data-consumer-state", "data-consumer-missing"],
        )
        self.assertEqual(len(trace.snapshots), 2)
        for snapshot in trace.snapshots:
            self.assertEqual(
                snapshot["canvas"]["attributes"],
                {"data-consumer-state": "opaque-value", "data-consumer-missing": None},
            )

    def test_canvas_identifier_validation_is_bounded_and_unique(self):
        self.assertEqual(validate_canvas_ids(["axial", "coronal"]), ("axial", "coronal"))
        with self.assertRaisesRegex(BrowserRuntimeError, "bounded HTML id"):
            validate_canvas_ids(["#axial"])
        with self.assertRaisesRegex(BrowserRuntimeError, "between one and eight"):
            validate_canvas_ids(3)
        with self.assertRaisesRegex(BrowserRuntimeError, "repeated"):
            validate_canvas_ids(["axial", "axial"])
        self.assertEqual(validate_canvas_attributes([]), ())
        self.assertEqual(validate_canvas_attributes(["data-consumer-state"]), ("data-consumer-state",))
        with self.assertRaisesRegex(BrowserRuntimeError, "bounded HTML name"):
            validate_canvas_attributes(["aria-label"])
        with self.assertRaisesRegex(BrowserRuntimeError, "repeated"):
            validate_canvas_attributes(["data-state", "data-state"])
        with self.assertRaisesRegex(BrowserRuntimeError, "at most"):
            validate_canvas_attributes([f"data-value-{index}" for index in range(MAX_CANVAS_ATTRIBUTES + 1)])
        with self.assertRaisesRegex(BrowserRuntimeError, "bounded HTML name"):
            validate_canvas_attributes(["data-" + "x" * 124])
        self.assertEqual(validate_consumer_revision("A" * 40), "a" * 40)
        with self.assertRaisesRegex(BrowserRuntimeError, "40-hex"):
            validate_consumer_revision("short")
        with self.assertRaisesRegex(BrowserRuntimeError, "40-hex"):
            validate_consumer_revision(3)

    def test_canvas_trace_rejects_malformed_attribute_responses(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        cases = (
            ({"data-requested": "ok", "data-extra": "unexpected"}, "invalid attribute snapshot"),
            ({"data-requested": 3}, "exceeds its value bound"),
            ({"data-requested": "x" * 1025}, "exceeds its value bound"),
        )
        for attributes, message in cases:
            with self.subTest(attributes=attributes):
                with tempfile.TemporaryDirectory(dir=output) as directory:
                    with self.assertRaisesRegex(BrowserRuntimeError, message):
                        run_canvas_scenario(
                            InvalidAttributeDriver(attributes),
                            BrowserEngine.CHROMIUM,
                            "http://127.0.0.1:8080/ritk.html",
                            "0" * 40,
                            pathlib.Path(directory),
                            5_000,
                            ["ritk-snap-axial"],
                            canvas_attributes=("data-requested",),
                        )

    def test_cli_rejects_consumer_attributes_on_workbench_scenario(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            trace_path = pathlib.Path(directory) / "invalid-cli.json"
            with mock.patch.object(
                sys,
                "argv",
                [
                    "browser_runtime.py",
                    "--engine",
                    "chromium",
                    "--scenario",
                    "workbench",
                    "--canvas-attribute",
                    "data-consumer-state",
                    "--output",
                    str(trace_path),
                ],
            ):
                self.assertEqual(main(), 1)
            document = json.loads(trace_path.read_text(encoding="utf-8"))
        self.assertEqual(document["status"], "failed")
        self.assertIn("requires --scenario canvas", document["error"])

    def test_cancel_trace_requires_pending_and_discards_delayed_result(self):
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
                True,
                4_000,
            )
        self.assertTrue(driver.closed)
        self.assertIn({"action": "submit", "state": "pending"}, trace.actions)
        self.assertIn({"action": "cancel-stop-remount", "stale_result": False}, trace.actions)
        self.assertFalse(any(action.get("state") == "success" for action in trace.actions))
        self.assertFalse(driver.pending)
        self.assertFalse(driver.success)

    def test_input_mutation_failure_is_observed(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            driver = NoInputDriver()
            with self.assertRaisesRegex(BrowserRuntimeError, "result-weight"):
                run_scenario(driver, BrowserEngine.CHROMIUM, "http://127.0.0.1:8080/", "disconnected", "0" * 40, pathlib.Path(directory), 5_000, False, 4_000)
        self.assertTrue(driver.closed)

    def test_teardown_failure_is_observed(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            driver = RetainingDriver()
            with self.assertRaisesRegex(BrowserRuntimeError, "stopped DOM"):
                run_scenario(driver, BrowserEngine.FIREFOX, "http://127.0.0.1:8080/", "disconnected", "0" * 40, pathlib.Path(directory), 5_000, False, 4_000)
        self.assertTrue(driver.closed)

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

    def test_screenshot_transport_and_decoding_bounds_are_enforced(self):
        image = _png_with_text(600 * 1024)
        response = JsonResponse(json.dumps({"value": base64.b64encode(image).decode()}).encode())
        client = browser_protocol.WebDriverClient("http://127.0.0.1:9515", 1)
        client.session_id = "session"
        with mock.patch.object(browser_protocol.urllib.request, "urlopen", return_value=response):
            self.assertEqual(client.screenshot(), image)
        self.assertGreater(response.read_limit, MAX_TRACE_BYTES)
        self.assertEqual(response.read_limit, MAX_SCREENSHOT_RESPONSE_BYTES + 1)

        element_response = JsonResponse(json.dumps({"value": base64.b64encode(_png()).decode()}).encode())
        requests = []

        def open_element(request, timeout):
            del timeout
            requests.append(request.full_url)
            return element_response

        client = browser_protocol.WebDriverClient("http://127.0.0.1:9515", 1)
        client.session_id = "session"
        with mock.patch.object(browser_protocol.urllib.request, "urlopen", side_effect=open_element):
            self.assertEqual(client.element_screenshot("opaque/id"), _png())
        self.assertEqual(requests, ["http://127.0.0.1:9515/session/session/element/opaque%2Fid/screenshot"])

        for value in ("not-base64", base64.b64encode(b"not png").decode()):
            malformed = JsonResponse(json.dumps({"value": value}).encode())
            client = browser_protocol.WebDriverClient("http://127.0.0.1:9515", 1)
            client.session_id = "session"
            with mock.patch.object(browser_protocol.urllib.request, "urlopen", return_value=malformed):
                with self.assertRaises(BrowserRuntimeError):
                    client.screenshot()

        oversized = _png_with_text(MAX_SCREENSHOT_BYTES)
        oversized_response = JsonResponse(json.dumps({"value": base64.b64encode(oversized).decode()}).encode())
        client = browser_protocol.WebDriverClient("http://127.0.0.1:9515", 1)
        client.session_id = "session"
        with mock.patch.object(browser_protocol.urllib.request, "urlopen", return_value=oversized_response):
            with self.assertRaises(BrowserRuntimeError):
                client.screenshot()

    def test_driver_escapes_opaque_element_identifiers(self):
        client = WebDriverClient("http://127.0.0.1:9515", 1)
        client.session_id = "session"
        requests = []

        def record(method, path, payload=None):
            requests.append((method, path, payload))
            return None

        client._request = record
        client.click("opaque/id")
        self.assertEqual(requests[0][1], "/session/session/element/opaque%2Fid/click")

    def test_driver_dispatches_bounded_pointer_and_wheel_actions(self):
        client = WebDriverClient("http://127.0.0.1:9515", 1)
        client.session_id = "session"
        requests = []

        def record(method, path, payload=None):
            requests.append((method, path, payload))
            return None

        client._request = record
        client.pointer_drag("opaque/id", (4, 8), (20, 28))
        client.wheel("opaque/id", (12, 16), (0, 120))
        client.release_actions()

        self.assertEqual(requests[0][0:2], ("POST", "/session/session/actions"))
        pointer = requests[0][2]["actions"][0]
        self.assertEqual(pointer["parameters"], {"pointerType": "mouse"})
        self.assertEqual(pointer["actions"][0]["origin"], {browser_protocol.ELEMENT_KEY: "opaque/id"})
        self.assertEqual(pointer["actions"][2]["x"], 16)
        self.assertEqual(requests[1][2]["actions"][0]["actions"][0]["deltaY"], 120)
        self.assertEqual(requests[2][0:2], ("DELETE", "/session/session/actions"))

    def test_driver_rejects_unbounded_or_malformed_actions(self):
        client = WebDriverClient("http://127.0.0.1:9515", 1)
        client.session_id = "session"
        client._request = lambda method, path, payload=None: None
        with self.assertRaisesRegex(BrowserRuntimeError, "source count"):
            client.perform_actions([])
        with self.assertRaisesRegex(BrowserRuntimeError, "source type"):
            client.perform_actions([{"type": "none", "id": "x", "actions": [{"type": "pause", "duration": 0}]}])
        with self.assertRaisesRegex(BrowserRuntimeError, "coordinates"):
            client.pointer_drag("canvas", (0, 0), (5000, 0))
        with self.assertRaisesRegex(BrowserRuntimeError, "deltas"):
            client.wheel("canvas", (0, 0), (0, 1_000_001))

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
