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
    MAX_LIFECYCLE_CYCLES,
    _write_trace,
    _validate_bridge_url,
    run_scenario,
)
from browser_runtime_cli import main
from browser_canvas import (
    KeyboardTraceKind,
    MAX_CANVAS_ATTRIBUTES,
    _canvas_action_offsets,
    capture_canvas_trace,
    ensure_canvas_visible,
    run_canvas_scenario,
    validate_canvas_attributes,
    validate_canvas_ids,
    validate_consumer_revision,
)
from browser_protocol import (
    BrowserRuntimeError,
    MAX_FILE_INPUT_PATHS,
    MAX_FILE_INPUT_VALUE_BYTES,
    MAX_FILE_PATH_BYTES,
    MAX_WINDOW_DIMENSION,
    MAX_SCREENSHOT_BYTES,
    MAX_SCREENSHOT_RESPONSE_BYTES,
    MAX_TRACE_BYTES,
    StaticServer,
    WebDriverClient,
    format_device_scale,
    parse_device_scale,
)
from browser_static_server import publish_port
from browser_trace import (
    BrowserEngine,
    Trace,
    browser_heap_sample,
    browser_memory_sample,
    record_device_scale,
)


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
        self.event_trace = {}
        self.event_types = ("pointerdown", "pointermove", "pointerup", "wheel")
        self.focused_canvas = None
        self.focus_calls = []
        self.held_keys = {}
        self.protocol_requests = []
        self.values = {"weight-kg": "72.5", "target-dose": "0.5"}
        self.generation = 1
        self.listener_count = 10
        self.device_scale_milli = 1000
        self.heap = {
            "used_js_heap_bytes": 1_048_576,
            "total_js_heap_bytes": 2_097_152,
            "js_heap_limit_bytes": 4_194_304,
        }
        self.memory_bytes = 8_388_608

    def create_session(self, browser_name: str, device_scale_milli=None) -> None:
        self.session_id = "session"
        self.device_scale_milli = device_scale_milli or 1000
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
            self.generation += 1
            self.listener_count = 0
        elif element_id == "metis-start":
            self.stopped = False
            self.success = False
            self.pending = False
            self.generation += 1
            self.listener_count = 10

    def _session_path(self, suffix: str) -> str:
        return f"/session/{self.session_id}/{suffix}"

    def _request(self, method, path, payload=None):
        self.protocol_requests.append((method, path, payload))
        if payload is None or payload.get("cmd") != "Input.dispatchKeyEvent":
            return None
        canvas_id = self.focused_canvas
        if canvas_id is None:
            return None
        params = payload["params"]
        self.event_trace.setdefault(canvas_id, []).append(
            {
                "type": params["type"].lower(),
                "is_trusted": True,
                "target_id": canvas_id,
                "client_x": None,
                "client_y": None,
                "delta_x": None,
                "delta_y": None,
                "key": params["key"],
                "code": params["code"],
                "repeat": params["autoRepeat"],
                "alt_key": False,
                "ctrl_key": False,
                "meta_key": False,
                "shift_key": False,
            }
        )
        return None

    def execute(self, script: str, arguments=()):
        if "matchMedia" in script and "focus_order" in script:
            return {
                "ok": True,
                "media": {
                    "reduced_motion": False,
                    "forced_colors": False,
                    "contrast_more": False,
                },
                "viewport": {
                    "width": 1280,
                    "height": 720,
                    "device_pixel_ratio": self.device_scale_milli / 1000,
                },
                "visual_viewport": {
                    "scale": 1.0,
                    "width": 1280.0,
                    "height": 720.0,
                },
                "document": {
                    "client_width": 1280,
                    "scroll_width": 1280,
                    "client_height": 720,
                    "scroll_height": 720,
                },
                "active_before": None,
                "active_after": None,
                "focus_order": [
                    {"id": "weight-kg", "role": "textbox", "name": "Weight (kg)"},
                ],
                "focus_sequence": ["weight-kg"],
                "geometry": {
                    "metis-app": {
                        "left": 0.0,
                        "top": 0.0,
                        "width": 960.0,
                        "height": 720.0,
                        "right": 960.0,
                        "bottom": 720.0,
                    },
                    "weight-kg": {
                        "left": 16.0,
                        "top": 16.0,
                        "width": 320.0,
                        "height": 44.0,
                        "right": 336.0,
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
                ],
            }
        if "devicePixelRatio" in script:
            return {
                "device_pixel_ratio": self.device_scale_milli / 1000,
                "inner_width": 1280,
                "inner_height": 720,
            }
        if "performance.memory" in script and "usedJSHeapSize" in script:
            return {"available": True, "source": "performance.memory", **self.heap}
        if "__metisCanvasTraceState" in script and "const ids = arguments[0]" in script:
            self.event_trace = {canvas_id: [] for canvas_id in arguments[0]}
            self.event_types = tuple(arguments[1])
            return {"ok": True, "listener_count": len(arguments[0]) * len(self.event_types)}
        if "__metisCanvasTraceState" in script and "state.events[id]" in script:
            canvas_id = arguments[0]
            events = self.event_trace.get(canvas_id)
            if events is None:
                return None
            self.event_trace[canvas_id] = []
            return {"events": events, "overflow": False}
        if "delete window.__metisCanvasTraceState" in script:
            listener_count = sum(1 for _ in self.event_trace for _ in self.event_types)
            self.event_trace = {}
            return {"ok": True, "listener_count": listener_count}
        if "document.activeElement" in script and "canvas.focus()" in script:
            self.focused_canvas = arguments[0]
            self.focus_calls.append(self.focused_canvas)
            return {"ok": True, "active_id": self.focused_canvas}
        if "scrollIntoView" in script:
            return {
                "ok": True,
                "error": None,
                "left": 0,
                "top": 0,
                "right": 512,
                "bottom": 512,
            }
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
        if "measureUserAgentSpecificMemory" in script:
            return {
                "available": True,
                "source": "performance.measureUserAgentSpecificMemory",
                "secure_context": True,
                "cross_origin_isolated": True,
                "estimated_bytes": self.memory_bytes,
            }
        if "requestAnimationFrame" in script:
            sample_count = arguments[0]
            return {"ok": True, "timestamps": [float(index * 16) for index in range(sample_count)]}
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
        self.event_trace.setdefault(element_id, []).extend(
            [
                {
                    "type": "pointerdown",
                    "is_trusted": True,
                    "target_id": element_id,
                    "client_x": 24,
                    "client_y": 24,
                    "delta_x": None,
                    "delta_y": None,
                },
                {
                    "type": "pointermove",
                    "is_trusted": True,
                    "target_id": element_id,
                    "client_x": 64,
                    "client_y": 48,
                    "delta_x": None,
                    "delta_y": None,
                },
                {
                    "type": "pointerup",
                    "is_trusted": True,
                    "target_id": element_id,
                    "client_x": 64,
                    "client_y": 48,
                    "delta_x": None,
                    "delta_y": None,
                },
            ]
        )

    def wheel(self, element_id, position, delta) -> None:
        self.canvas_actions.append(("wheel", element_id, position, delta))
        self.event_trace.setdefault(element_id, []).append(
            {
                "type": "wheel",
                "is_trusted": True,
                "target_id": element_id,
                "client_x": 64,
                "client_y": 48,
                "delta_x": 0,
                "delta_y": 120,
            }
        )

    def key_press(self, key: str) -> None:
        self.canvas_actions.append(("key-press", key))
        canvas_id = self.focused_canvas
        if canvas_id is None:
            return
        for event_type in ("keydown", "keyup"):
            self.event_trace.setdefault(canvas_id, []).append(
                {
                    "type": event_type,
                    "is_trusted": True,
                    "target_id": canvas_id,
                    "client_x": None,
                    "client_y": None,
                    "delta_x": None,
                    "delta_y": None,
                    "key": key,
                    "code": "Equal" if key == "=" else key,
                    "repeat": False,
                    "alt_key": False,
                    "ctrl_key": False,
                    "meta_key": False,
                    "shift_key": False,
                }
            )

    def perform_actions(self, actions) -> None:
        self.canvas_actions.append(("perform-actions", actions))
        canvas_id = self.focused_canvas
        if canvas_id is None:
            return
        for source in actions:
            held = self.held_keys.setdefault(source["id"], set())
            for action in source["actions"]:
                key = action["value"]
                if action["type"] == "keyDown":
                    repeat = key in held
                    held.add(key)
                    event_type = "keydown"
                else:
                    repeat = False
                    held.discard(key)
                    event_type = "keyup"
                self.event_trace.setdefault(canvas_id, []).append(
                    {
                        "type": event_type,
                        "is_trusted": True,
                        "target_id": canvas_id,
                        "client_x": None,
                        "client_y": None,
                        "delta_x": None,
                        "delta_y": None,
                        "key": key,
                        "code": {"=": "Equal", "-": "Minus"}[key],
                        "repeat": repeat,
                        "alt_key": False,
                        "ctrl_key": False,
                        "meta_key": False,
                        "shift_key": False,
                    }
                )

    def release_actions(self) -> None:
        self.released = True

    def snapshot(self):
        if self.stopped:
            return {
                "app_text": f"Lifecycle: stopped; Rust-owned listeners released (0 listener handles; generation {self.generation})Metis browser host stopped.",
                "mounted_controls": 0,
                "lifecycle": {"listener_count": 0, "generation": self.generation},
                "elements": {},
            }
        state = "Backend result received" if self.success else "Request in progress" if self.pending else "Browser controls are active."
        metrics = "Volume rate: 0.900000 mL/hr" if self.success else ""
        return {
            "app_text": "Authorized clinical form boundary",
            "mounted_controls": 12,
            "lifecycle": {"listener_count": self.listener_count, "generation": self.generation},
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


class UntrustedCanvasDriver(FakeDriver):
    """Driver mutant that reports a synthetic, untrusted pointer event."""

    def pointer_drag(self, element_id, start, end) -> None:
        super().pointer_drag(element_id, start, end)
        self.event_trace[element_id][0]["is_trusted"] = False


class IncompleteCanvasDriver(FakeDriver):
    """Driver mutant that drops pointer phases from the trusted action."""

    def pointer_drag(self, element_id, start, end) -> None:
        super().pointer_drag(element_id, start, end)
        self.event_trace[element_id] = self.event_trace[element_id][:1]


class NonRepeatingKeyboardDriver(FakeDriver):
    """Driver mutant that omits W3C held-key repeat metadata."""

    def perform_actions(self, actions) -> None:
        super().perform_actions(actions)
        for event in self.event_trace.get(self.focused_canvas, []):
            if event["type"] == "keydown":
                event["repeat"] = False


class IncorrectInitialDevtoolsRepeatDriver(FakeDriver):
    """Driver mutant that reports repeat on the first DevTools keydown."""

    def _request(self, method, path, payload=None):
        result = super()._request(method, path, payload)
        if (
            payload is not None
            and payload.get("cmd") == "Input.dispatchKeyEvent"
            and payload["params"]["type"] == "keyDown"
            and payload["params"]["autoRepeat"] is False
        ):
            self.event_trace[self.focused_canvas][-1]["repeat"] = True
        return result


class FailingDevtoolsCleanupDriver(IncorrectInitialDevtoolsRepeatDriver):
    """Driver mutant that rejects cleanup after invalid repeat evidence."""

    def _request(self, method, path, payload=None):
        if (
            payload is not None
            and payload.get("cmd") == "Input.dispatchKeyEvent"
            and payload["params"]["type"] == "keyUp"
        ):
            raise BrowserRuntimeError("injected DevTools cleanup failure")
        return super()._request(method, path, payload)


class FailingListenerCleanupDriver(IncorrectInitialDevtoolsRepeatDriver):
    """Driver mutant that rejects listener cleanup after a trace failure."""

    def execute(self, script: str, arguments=()):
        if "delete window.__metisCanvasTraceState" in script:
            raise BrowserRuntimeError("injected listener cleanup failure")
        return super().execute(script, arguments)


class InvalidFrameTimingDriver(FakeDriver):
    """Driver mutant that reports a non-monotonic animation-frame sample."""

    def execute_async(self, script: str, arguments=()):
        if "requestAnimationFrame" in script:
            sample_count = arguments[0]
            return {"ok": True, "timestamps": [float(index) for index in range(sample_count - 1, -1, -1)]}
        return super().execute_async(script, arguments)


class UnavailableHeapDriver(FakeDriver):
    """Driver mutant that reports an unsupported browser heap surface."""

    def execute(self, script: str, arguments=()):
        if "performance.memory" in script and "usedJSHeapSize" in script:
            return {"available": False, "reason": "performance.memory unavailable"}
        return super().execute(script, arguments)


class InvalidHeapDriver(FakeDriver):
    """Driver mutant that violates the browser heap ordering contract."""

    def execute(self, script: str, arguments=()):
        value = super().execute(script, arguments)
        if "performance.memory" in script and "usedJSHeapSize" in script:
            value["total_js_heap_bytes"] = value["used_js_heap_bytes"] - 1
        return value


class UnavailableMemoryDriver(FakeDriver):
    """Driver mutant that reports an unsupported aggregate memory surface."""

    def execute_async(self, script: str, arguments=()):
        if "measureUserAgentSpecificMemory" in script:
            return {"available": False, "reason": "cross-origin isolation required"}
        return super().execute_async(script, arguments)


class FailedMemoryDriver(FakeDriver):
    """Driver mutant that reports a rejected or timed-out memory probe."""

    def __init__(self, reason: str) -> None:
        super().__init__()
        self.reason = reason

    def execute_async(self, script: str, arguments=()):
        if "measureUserAgentSpecificMemory" in script:
            return {"available": False, "reason": self.reason}
        return super().execute_async(script, arguments)


class InvalidMemoryDriver(FakeDriver):
    """Driver mutant that violates the aggregate memory value contract."""

    def execute_async(self, script: str, arguments=()):
        if "measureUserAgentSpecificMemory" in script:
            return {
                "available": True,
                "source": "performance.measureUserAgentSpecificMemory",
                "secure_context": True,
                "cross_origin_isolated": True,
                "estimated_bytes": float("nan"),
            }
        return super().execute_async(script, arguments)


class RetainingDriver(FakeDriver):
    """Driver mutant that leaves controls mounted after the stop command."""

    def snapshot(self):
        snapshot = super().snapshot()
        if self.stopped:
            snapshot["mounted_controls"] = 12
        else:
            snapshot["elements"]["submit-calculation"]["disabled"] = True
        return snapshot


class EmptyRemountDriver(FakeDriver):
    """Driver mutant that restores listeners without mounting controls."""

    def snapshot(self):
        snapshot = super().snapshot()
        if not self.stopped:
            snapshot["mounted_controls"] = 0
        return snapshot


class KeyboardSubmitDriver(FakeDriver):
    """Driver that models DOM focus traversal and an Enter activation."""

    def __init__(self) -> None:
        super().__init__()
        self.active_element = None
        self.keyboard_actions = []

    def execute(self, script: str, arguments=()):
        if "const element = document.getElementById(arguments[0]);" in script:
            element_id = arguments[0]
            if element_id not in {"weight-kg", "submit-calculation"}:
                return False
            self.active_element = element_id
            return True
        if "const element = document.activeElement;" in script:
            return self.active_element
        if "matchMedia" in script and "focus_order" in script:
            snapshot = super().execute(script, arguments)
            snapshot["focus_order"] = [
                {"id": "weight-kg", "role": "textbox", "name": "Weight (kg)"},
                {"id": "submit-calculation", "role": "button", "name": "Submit"},
            ]
            snapshot["focus_sequence"] = ["weight-kg", "submit-calculation"]
            snapshot["geometry"]["submit-calculation"] = {
                "left": 16.0,
                "top": 72.0,
                "width": 160.0,
                "height": 44.0,
                "right": 176.0,
                "bottom": 116.0,
            }
            return snapshot
        return super().execute(script, arguments)

    def key_press(self, key: str) -> None:
        self.keyboard_actions.append(key)
        if key == "Tab" and self.active_element == "weight-kg":
            self.active_element = "submit-calculation"
        elif key == "Enter" and self.active_element == "submit-calculation":
            self.pending = True
            self.success = False


class DisabledKeyboardSubmitDriver(KeyboardSubmitDriver):
    """Driver mutant that exposes a disabled submit target in the AX snapshot."""

    def execute(self, script: str, arguments=()):
        snapshot = super().execute(script, arguments)
        if "matchMedia" in script and "focus_order" in script and isinstance(snapshot, dict):
            for semantic in snapshot["semantics"]:
                if semantic["id"] == "submit-calculation":
                    semantic["states"]["disabled"] = True
        return snapshot


class BrowserRuntimeTests(unittest.TestCase):
    """The same trace keeps its value semantics across all engine names."""

    def test_engine_names_are_closed_and_webdriver_capabilities_are_stable(self):
        self.assertEqual(BrowserEngine.parse("CHROMIUM"), BrowserEngine.CHROMIUM)
        self.assertEqual(BrowserEngine.FIREFOX.webdriver_name, "firefox")
        self.assertEqual(BrowserEngine.WEBKIT.webdriver_name, "safari")
        self.assertEqual(BrowserEngine.CHROMIUM.resolve_webdriver_name(), "chrome")
        self.assertEqual(BrowserEngine.CHROMIUM.resolve_webdriver_name("MicrosoftEdge"), "MicrosoftEdge")
        with self.assertRaisesRegex(BrowserRuntimeError, "incompatible with firefox"):
            BrowserEngine.FIREFOX.resolve_webdriver_name("MicrosoftEdge")
        with self.assertRaises(BrowserRuntimeError):
            BrowserEngine.parse("blink")

    def test_device_scale_parser_uses_bounded_fixed_point(self):
        self.assertEqual(parse_device_scale("2"), 2_000)
        self.assertEqual(parse_device_scale("1.25"), 1_250)
        self.assertEqual(format_device_scale(1_250), "1.25")
        for value in ("0.499", "4.001", "1.2345", "NaN", "1e0", ""):
            with self.subTest(value=value):
                with self.assertRaisesRegex(BrowserRuntimeError, "device scale"):
                    parse_device_scale(value)

    def test_window_rectangle_is_bounded_and_sent_once(self):
        client = WebDriverClient("http://127.0.0.1:9515", 1)
        client.session_id = "session"
        with mock.patch.object(client, "_request", return_value=None) as request:
            client.set_window_rect(1440, 1200)
        request.assert_called_once_with(
            "POST",
            "/session/session/window/rect",
            {"width": 1440, "height": 1200},
        )
        for width, height in ((0, 1), (1, 0), (MAX_WINDOW_DIMENSION + 1, 1), (1, MAX_WINDOW_DIMENSION + 1), (True, 1)):
            with self.assertRaisesRegex(BrowserRuntimeError, "window dimensions"):
                client.set_window_rect(width, height)

    def test_driver_emits_engine_specific_device_scale_capabilities(self):
        for browser_name, option_name in (
            ("chrome", "goog:chromeOptions"),
            ("MicrosoftEdge", "ms:edgeOptions"),
        ):
            requests = []
            client = WebDriverClient("http://127.0.0.1:9515", 1)

            def request(method, path, payload=None):
                requests.append((method, path, payload))
                return {"sessionId": "session", "capabilities": {}}

            client._request = request
            client.create_session(browser_name, 2_000)
            capabilities = requests[0][2]["capabilities"]["alwaysMatch"]
            self.assertEqual(capabilities[option_name]["args"], ["--force-device-scale-factor=2"])

        requests = []
        client = WebDriverClient("http://127.0.0.1:9515", 1)
        client._request = lambda method, path, payload=None: requests.append(payload) or {
            "sessionId": "session",
            "capabilities": {},
        }
        client.create_session("firefox", 1_250)
        self.assertEqual(
            requests[0]["capabilities"]["alwaysMatch"]["moz:firefoxOptions"],
            {"prefs": {"layout.css.devPixelsPerPx": "1.25"}},
        )
        with self.assertRaisesRegex(BrowserRuntimeError, "WebKit.*override"):
            client.create_session("safari", 2_000)

    def test_device_scale_probe_records_effective_value_and_viewport(self):
        trace = Trace(BrowserEngine.CHROMIUM, "http://127.0.0.1/", "canvas", "0" * 40, {})
        driver = FakeDriver()
        driver.create_session("chrome", 2_000)
        measurement = record_device_scale(driver, trace, 2_000)
        self.assertEqual(measurement["requested"], "2")
        self.assertEqual(measurement["effective_milli"], 2_000)
        self.assertEqual(measurement["css_viewport"], {"width": 1280, "height": 720})

    def test_device_scale_probe_rejects_requested_mismatch(self):
        driver = FakeDriver()
        driver.device_scale_milli = 1_000
        trace = Trace(BrowserEngine.CHROMIUM, "http://127.0.0.1/", "canvas", "0" * 40, {})
        with self.assertRaisesRegex(BrowserRuntimeError, "differs from requested"):
            record_device_scale(driver, trace, 2_000)

    def test_device_scale_probe_rejects_malformed_browser_values(self):
        trace = Trace(BrowserEngine.CHROMIUM, "http://127.0.0.1/", "canvas", "0" * 40, {})
        driver = FakeDriver()
        driver.execute = lambda script, arguments=(): {"device_pixel_ratio": "2", "inner_width": 1280, "inner_height": 720}
        with self.assertRaisesRegex(BrowserRuntimeError, "invalid devicePixelRatio"):
            record_device_scale(driver, trace)
        driver.execute = lambda script, arguments=(): {"device_pixel_ratio": 2.0, "inner_width": 0, "inner_height": 720}
        with self.assertRaisesRegex(BrowserRuntimeError, "invalid CSS viewport"):
            record_device_scale(driver, trace)

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
                device_scale_milli=2_000,
            )
        self.assertTrue(driver.closed)
        self.assertEqual(driver.capabilities["browserName"], "chrome")
        self.assertEqual([action["field"] for action in trace.actions if action["action"] == "input-change"], ["weight-kg", "target-dose"])
        self.assertEqual(trace.actions[-1], {"action": "stop-remount", "stale_result": False})
        self.assertEqual(trace.cleanup["stopped_mounted_controls"], 0)
        self.assertEqual(trace.cleanup["remounted_mounted_controls"], 12)
        self.assertEqual(trace.cleanup["stopped_listener_count"], 0)
        self.assertEqual(trace.cleanup["remounted_listener_count"], 10)
        self.assertEqual(trace.cleanup["remounted_generation"], trace.cleanup["stopped_generation"] + 1)
        self.assertEqual(trace.cleanup["pending_requests"], 0)
        self.assertEqual(trace.cleanup["pending_request_observation"], "remounted metis-form aria-busy=false")
        self.assertEqual(trace.metrics["device_scale"]["effective"], "2")
        self.assertEqual(len(trace.screenshots), 6)
        document = Trace(BrowserEngine.FIREFOX, "http://127.0.0.1/", "disconnected", "0" * 40, {}).document()
        self.assertEqual(document["schema"], 1)
        self.assertEqual(document["unsupported_native_operations"], ["native-file-dialog", "native-process-launch", "os-permission-grant"])

    def test_repeated_lifecycle_trace_records_each_bounded_cycle(self):
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
                browser_heap=True,
                browser_memory=True,
                lifecycle_cycles=3,
            )
        records = trace.metrics["lifecycle_cycles"]
        self.assertEqual([record["cycle"] for record in records], [1, 2, 3])
        self.assertEqual(trace.cleanup["lifecycle_cycles"], 3)
        self.assertEqual(trace.cleanup["final_generation"], records[-1]["remounted"]["generation"])
        for record in records:
            self.assertEqual(record["stopped"]["mounted_controls"], 0)
            self.assertEqual(record["stopped"]["listener_count"], 0)
            self.assertGreater(record["remounted"]["mounted_controls"], 0)
            self.assertGreater(record["remounted"]["listener_count"], 0)
            self.assertEqual(
                record["remounted"]["generation"],
                record["stopped"]["generation"] + 1,
            )
        heap_labels = [sample["label"] for sample in trace.metrics["browser_heap"]]
        self.assertEqual(
            heap_labels,
            [
                "initial",
                "after-weight-kg",
                "after-target-dose",
                "success",
                "remounted",
                "remounted-cycle-2",
                "remounted-cycle-3",
            ],
        )
        memory_labels = [sample["label"] for sample in trace.metrics["browser_memory"]]
        self.assertEqual(memory_labels, heap_labels)
        self.assertTrue(all(sample["available"] for sample in trace.metrics["browser_memory"]))
        self.assertEqual(len(trace.screenshots), 6)

    def test_accessibility_probe_records_focus_media_and_geometry(self):
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
                accessibility_probe=True,
            )
        records = trace.metrics["accessibility"]
        self.assertEqual(
            [record["label"] for record in records],
            ["initial", "after-weight-kg", "after-target-dose", "remounted"],
        )
        for record in records:
            self.assertEqual(record["media"], {"reduced_motion": False, "forced_colors": False, "contrast_more": False})
            self.assertEqual(record["viewport"], {"width": 1280, "height": 720, "device_pixel_ratio": 1.0})
            self.assertEqual(record["focus_sequence"], ["weight-kg"])
            self.assertEqual(record["geometry"]["weight-kg"]["width"], 320.0)
            self.assertIsNone(record["active_after"])

    def test_accessibility_probe_precedes_detached_feature_probe(self):
        """Keep Chromium's native tree capture ahead of temporary probe nodes."""
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        events = []

        def record_accessibility(*args, **kwargs):
            del args, kwargs
            events.append("accessibility")
            return {}

        def record_features(*args, **kwargs):
            del args, kwargs
            events.append("features")

        with mock.patch("browser_runtime.capture_accessibility", side_effect=record_accessibility):
            with mock.patch("browser_runtime.capture_runtime_features", side_effect=record_features):
                with tempfile.TemporaryDirectory(dir=output) as directory:
                    run_scenario(
                        FakeDriver(),
                        BrowserEngine.CHROMIUM,
                        "http://127.0.0.1:8080/?endpoint=ws%3A%2F%2F127.0.0.1%3A8765%2Fsocket&process=42&principal=66666666666666666666666666666666",
                        "authorized",
                        "0" * 40,
                        pathlib.Path(directory),
                        5_000,
                        False,
                        4_000,
                        accessibility_probe=True,
                    )

        self.assertEqual(events[:2], ["accessibility", "features"])

    def test_keyboard_submit_traverses_focus_order_and_completes(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            driver = KeyboardSubmitDriver()
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
                accessibility_probe=True,
                keyboard_submit=True,
            )
        keyboard_action = next(action for action in trace.actions if action["action"] == "keyboard-submit")
        self.assertEqual(keyboard_action["key"], "Enter")
        self.assertEqual(keyboard_action["focus_path"], ["weight-kg", "submit-calculation"])
        self.assertEqual(driver.keyboard_actions, ["Tab", "Enter"])
        self.assertEqual(
            [record["label"] for record in trace.metrics["accessibility"]],
            ["initial", "after-weight-kg", "after-target-dose", "after-keyboard-submit", "remounted"],
        )
        self.assertEqual(
            next(action for action in trace.actions if action["action"] == "submit")["state"],
            "success",
        )

    def test_keyboard_submit_requires_authorized_accessibility_trace(self):
        common = (
            FakeDriver(),
            BrowserEngine.CHROMIUM,
            "http://127.0.0.1:8080/",
            "disconnected",
            "0" * 40,
            pathlib.Path("."),
            5_000,
            False,
            4_000,
        )
        with self.assertRaisesRegex(BrowserRuntimeError, "requires --accessibility-probe"):
            run_scenario(*common, keyboard_submit=True)
        with self.assertRaisesRegex(BrowserRuntimeError, "requires --bridge authorized"):
            run_scenario(*common, accessibility_probe=True, keyboard_submit=True)

    def test_keyboard_submit_rejects_missing_or_disabled_targets(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        cases = (
            (FakeDriver(), "target is absent from the observed focus order"),
            (DisabledKeyboardSubmitDriver(), "target is disabled"),
        )
        for driver, message in cases:
            with self.subTest(message=message), tempfile.TemporaryDirectory(dir=output) as directory:
                with self.assertRaisesRegex(BrowserRuntimeError, message):
                    run_scenario(
                        driver,
                        BrowserEngine.CHROMIUM,
                        "http://127.0.0.1:8080/?endpoint=ws%3A%2F%2F127.0.0.1%3A8765%2Fsocket&process=42&principal=66666666666666666666666666666666",
                        "authorized",
                        "0" * 40,
                        pathlib.Path(directory),
                        5_000,
                        False,
                        4_000,
                        accessibility_probe=True,
                        keyboard_submit=True,
                    )

    def test_lifecycle_cycle_bound_is_enforced(self):
        for value in (0, MAX_LIFECYCLE_CYCLES + 1, True):
            with self.subTest(value=value):
                with self.assertRaisesRegex(BrowserRuntimeError, "lifecycle-cycles must be between"):
                    run_scenario(
                        FakeDriver(),
                        BrowserEngine.CHROMIUM,
                        "http://127.0.0.1:8080/",
                        "disconnected",
                        "0" * 40,
                        pathlib.Path("."),
                        5_000,
                        False,
                        4_000,
                        lifecycle_cycles=value,
                    )

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
                ["viewer-axial", "viewer-coronal", "viewer-sagittal"],
                "1" * 40,
                browser_heap=True,
                browser_memory=True,
                browser_name="MicrosoftEdge",
            )
        self.assertTrue(driver.closed)
        self.assertEqual(driver.capabilities["browserName"], "MicrosoftEdge")
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
        for pointer_action, wheel_action in zip(trace.actions[::2], trace.actions[1::2]):
            self.assertTrue(pointer_action["observed_events"])
            self.assertTrue(all(event["is_trusted"] for event in pointer_action["observed_events"]))
            self.assertEqual(
                {event["type"] for event in pointer_action["observed_events"]},
                {"pointerdown", "pointermove", "pointerup"},
            )
            self.assertEqual(
                {event["type"] for event in wheel_action["observed_events"]},
                {"wheel"},
            )
            self.assertTrue(all(event["is_trusted"] for event in wheel_action["observed_events"]))
        self.assertEqual(len(trace.snapshots), 6)
        self.assertEqual(len(trace.screenshots), 8)
        self.assertTrue(all(item["scope"] == "element" for item in trace.screenshots[1:-1]))
        self.assertEqual(trace.cleanup["diagnostic_listener_count"], 12)
        self.assertTrue(trace.cleanup["diagnostic_listeners_released"])
        frame_intervals = trace.metrics["frame_intervals"]
        self.assertEqual(len(frame_intervals), 6)
        self.assertTrue(all(item["sample_count"] == 7 for item in frame_intervals))
        self.assertTrue(all(item["mean_ms"] == 16.0 for item in frame_intervals))
        heap = trace.metrics["browser_heap"]
        self.assertEqual(len(heap), 6)
        self.assertTrue(all(item["available"] for item in heap))
        self.assertTrue(all(item["used_js_heap_bytes"] <= item["total_js_heap_bytes"] <= item["js_heap_limit_bytes"] for item in heap))
        memory = trace.metrics["browser_memory"]
        self.assertEqual(len(memory), 6)
        self.assertTrue(all(item["available"] for item in memory))
        self.assertTrue(all(item["estimated_bytes"] == 8_388_608 for item in memory))

    def test_canvas_trace_records_focused_trusted_keyboard_actions(self):
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
                ["viewer-axial", "viewer-coronal", "viewer-sagittal"],
                "1" * 40,
                keyboard_trace=KeyboardTraceKind.NAVIGATION,
            )
        self.assertTrue(driver.released)
        self.assertEqual(driver.event_types, ("pointerdown", "pointermove", "pointerup", "wheel", "keydown", "keyup"))
        self.assertEqual(trace.cleanup["diagnostic_listener_count"], 18)
        self.assertEqual(
            [action["action"] for action in trace.actions],
            ["trusted-keyboard", "trusted-pointer-drag", "trusted-wheel"] * 3,
        )
        for action in trace.actions[::3]:
            self.assertEqual(action["key"], KeyboardTraceKind.NAVIGATION.key)
            self.assertEqual(action["code"], KeyboardTraceKind.NAVIGATION.code)
            self.assertFalse(action["repeat"])
            self.assertEqual(action["transport"], "webdriver-actions")
            self.assertEqual(action["focus"], {"ok": True, "active_id": action["canvas"]})
            self.assertEqual(
                {event["type"] for event in action["observed_events"]},
                {"keydown", "keyup"},
            )
            self.assertTrue(all(event["is_trusted"] for event in action["observed_events"]))
            self.assertTrue(
                all(
                    event["key"] == KeyboardTraceKind.NAVIGATION.key
                    and event["code"] == KeyboardTraceKind.NAVIGATION.code
                    for event in action["observed_events"]
                )
            )
        self.assertEqual(len(trace.snapshots), 9)
        for canvas_id in ("viewer-axial", "viewer-coronal", "viewer-sagittal"):
            self.assertEqual(
                [snapshot["label"] for snapshot in trace.snapshots if snapshot["canvas"]["id"] == canvas_id],
                [f"{canvas_id}-initial", f"{canvas_id}-after-keyboard", f"{canvas_id}-after-input"],
            )

    def test_canvas_trace_records_cine_rate_keyboard_profile(self):
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
                ["viewer-axial"],
                "1" * 40,
                keyboard_trace=KeyboardTraceKind.CINE_RATE,
            )
        keyboard_actions = trace.actions[:4]
        self.assertEqual(
            [
                (action["key"], action["code"], action["repeat"])
                for action in keyboard_actions
            ],
            [
                ("=", "Equal", False),
                ("=", "Equal", True),
                ("-", "Minus", False),
                ("-", "Minus", True),
            ],
        )
        self.assertEqual(
            [
                [(event["type"], event["repeat"]) for event in action["observed_events"]]
                for action in keyboard_actions
            ],
            [
                [("keydown", False)],
                [("keydown", True), ("keyup", False)],
                [("keydown", False)],
                [("keydown", True), ("keyup", False)],
            ],
        )
        self.assertTrue(
            all(
                event["key"] == action["key"] and event["code"] == action["code"]
                for action in keyboard_actions
                for event in action["observed_events"]
            )
        )
        self.assertEqual(
            [action["transport"] for action in keyboard_actions],
            ["chromium-devtools"] * 4,
        )
        self.assertEqual(
            [request[1] for request in driver.protocol_requests],
            ["/session/session/goog/cdp/execute"] * 6,
        )
        self.assertEqual(
            [request[2]["params"] for request in driver.protocol_requests],
            [
                {"type": "keyDown", "key": "=", "code": "Equal", "windowsVirtualKeyCode": 187, "autoRepeat": False},
                {"type": "keyDown", "key": "=", "code": "Equal", "windowsVirtualKeyCode": 187, "autoRepeat": True},
                {"type": "keyUp", "key": "=", "code": "Equal", "windowsVirtualKeyCode": 187, "autoRepeat": False},
                {"type": "keyDown", "key": "-", "code": "Minus", "windowsVirtualKeyCode": 189, "autoRepeat": False},
                {"type": "keyDown", "key": "-", "code": "Minus", "windowsVirtualKeyCode": 189, "autoRepeat": True},
                {"type": "keyUp", "key": "-", "code": "Minus", "windowsVirtualKeyCode": 189, "autoRepeat": False},
            ],
        )
        self.assertTrue(
            all(request[2]["cmd"] == "Input.dispatchKeyEvent" for request in driver.protocol_requests)
        )
        self.assertEqual(driver.focus_calls, ["viewer-axial"] * 4)
        self.assertEqual(
            [snapshot["label"] for snapshot in trace.snapshots],
            [
                "viewer-axial-initial",
                "viewer-axial-after-keyboard",
                "viewer-axial-after-repeat",
                "viewer-axial-after-decrease",
                "viewer-axial-after-decrease-repeat",
                "viewer-axial-after-input",
            ],
        )
        self.assertEqual(
            [screenshot["label"] for screenshot in trace.screenshots],
            [
                "window-initial",
                "viewer-axial-initial",
                "viewer-axial-after-keyboard",
                "viewer-axial-after-repeat",
                "viewer-axial-after-decrease",
                "viewer-axial-after-decrease-repeat",
                "viewer-axial-after-input",
                "window-final",
            ],
        )
        self.assertEqual(
            [action["action"] for action in trace.actions],
            ["trusted-keyboard"] * 4 + ["trusted-pointer-drag", "trusted-wheel"],
        )
        self.assertTrue(
            all(
                action["focus"] == {"ok": True, "active_id": action["canvas"]}
                for action in keyboard_actions
            )
        )
        self.assertEqual(trace.cleanup["canvas_attribute_names"], [])

    def test_canvas_trace_uses_edge_devtools_endpoint_for_cine_rate(self):
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
                ["viewer-axial"],
                "1" * 40,
                keyboard_trace=KeyboardTraceKind.CINE_RATE,
                browser_name="MicrosoftEdge",
            )
        self.assertEqual(
            [request[1] for request in driver.protocol_requests],
            ["/session/session/ms/cdp/execute"] * 6,
        )
        self.assertEqual(
            [action["transport"] for action in trace.actions[:4]],
            ["chromium-devtools"] * 4,
        )

    def test_canvas_trace_retains_webdriver_cine_rate_transport_for_firefox(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            driver = FakeDriver()
            trace = run_canvas_scenario(
                driver,
                BrowserEngine.FIREFOX,
                "http://127.0.0.1:8080/ritk.html",
                "0" * 40,
                pathlib.Path(directory),
                5_000,
                ["viewer-axial"],
                "1" * 40,
                keyboard_trace=KeyboardTraceKind.CINE_RATE,
            )
        action_requests = [entry for entry in driver.canvas_actions if entry[0] == "perform-actions"]
        self.assertEqual(driver.protocol_requests, [])
        self.assertEqual([entry[1][0]["id"] for entry in action_requests], ["metis-keyboard"] * 4)
        self.assertEqual(
            [[item["type"] for item in entry[1][0]["actions"]] for entry in action_requests],
            [["keyDown"], ["keyDown", "keyUp"], ["keyDown"], ["keyDown", "keyUp"]],
        )
        self.assertEqual(
            [action["transport"] for action in trace.actions[:4]],
            ["webdriver-actions"] * 4,
        )

    def test_canvas_trace_rejects_missing_webdriver_repeat_evidence(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            with self.assertRaisesRegex(BrowserRuntimeError, "browser keyboard events reported"):
                run_canvas_scenario(
                    NonRepeatingKeyboardDriver(),
                    BrowserEngine.FIREFOX,
                    "http://127.0.0.1:8080/ritk.html",
                    "0" * 40,
                    pathlib.Path(directory),
                    5_000,
                    ["viewer-axial"],
                    "1" * 40,
                    keyboard_trace=KeyboardTraceKind.CINE_RATE,
                )

    def test_canvas_trace_retains_invalid_repeat_and_releases_held_devtools_key(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            driver = IncorrectInitialDevtoolsRepeatDriver()
            driver.create_session("chrome")
            trace = Trace(
                BrowserEngine.CHROMIUM,
                "http://127.0.0.1:8080/ritk.html",
                "canvas",
                "0" * 40,
                driver.capabilities,
                "1" * 40,
            )
            with self.assertRaisesRegex(BrowserRuntimeError, "browser keyboard events reported"):
                capture_canvas_trace(
                    driver,
                    trace,
                    pathlib.Path(directory),
                    ["viewer-axial"],
                    keyboard_trace=KeyboardTraceKind.CINE_RATE,
                )
        self.assertEqual(len(trace.actions), 1)
        self.assertEqual(trace.actions[0]["transport"], "chromium-devtools")
        self.assertEqual(trace.actions[0]["observed_events"][0]["repeat"], True)
        self.assertEqual(
            [request[2]["params"]["type"] for request in driver.protocol_requests],
            ["keyDown", "keyUp"],
        )
        self.assertEqual(driver.protocol_requests[-1][2]["params"]["autoRepeat"], False)
        self.assertTrue(driver.released)

    def test_canvas_trace_preserves_primary_error_when_devtools_cleanup_fails(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            driver = FailingDevtoolsCleanupDriver()
            driver.create_session("chrome")
            trace = Trace(
                BrowserEngine.CHROMIUM,
                "http://127.0.0.1:8080/ritk.html",
                "canvas",
                "0" * 40,
                driver.capabilities,
                "1" * 40,
            )
            with self.assertRaisesRegex(
                BrowserRuntimeError,
                "browser keyboard events reported",
            ) as captured:
                capture_canvas_trace(
                    driver,
                    trace,
                    pathlib.Path(directory),
                    ["viewer-axial"],
                    keyboard_trace=KeyboardTraceKind.CINE_RATE,
                )
        self.assertIn(
            "CDP key release also failed: injected DevTools cleanup failure",
            captured.exception.__notes__,
        )

    def test_canvas_trace_releases_actions_when_listener_cleanup_fails(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            driver = FailingListenerCleanupDriver()
            driver.create_session("chrome")
            trace = Trace(
                BrowserEngine.CHROMIUM,
                "http://127.0.0.1:8080/ritk.html",
                "canvas",
                "0" * 40,
                driver.capabilities,
                "1" * 40,
            )
            with self.assertRaisesRegex(
                BrowserRuntimeError,
                "browser keyboard events reported",
            ) as captured:
                capture_canvas_trace(
                    driver,
                    trace,
                    pathlib.Path(directory),
                    ["viewer-axial"],
                    keyboard_trace=KeyboardTraceKind.CINE_RATE,
                )
        self.assertTrue(driver.released)
        self.assertIn(
            "canvas event listener cleanup also failed: injected listener cleanup failure",
            captured.exception.__notes__,
        )

    def test_canvas_action_offsets_stay_inside_short_surfaces(self):
        offsets = _canvas_action_offsets({"css_width": 448.8, "css_height": 82.4})
        self.assertEqual(offsets, ((24, 24), (64, 40), (64, 40)))

        tiny = _canvas_action_offsets({"css_width": 1.0, "css_height": 1.0})
        self.assertEqual(tiny, ((0, 0), (0, 0), (0, 0)))

    def test_canvas_capture_scrolls_named_surface_into_view(self):
        class Client:
            def __init__(self):
                self.calls = []

            def execute(self, script, arguments):
                self.calls.append((script, arguments))
                return {
                    "ok": True,
                    "error": None,
                    "left": 10,
                    "top": 10,
                    "right": 522,
                    "bottom": 522,
                }

        client = Client()
        ensure_canvas_visible(client, "viewer-axial")
        self.assertEqual(len(client.calls), 1)
        self.assertEqual(client.calls[0][1], ["viewer-axial"])
        self.assertIn("scrollIntoView", client.calls[0][0])

    def test_canvas_capture_rejects_surface_that_remains_clipped(self):
        class Client:
            def execute(self, _script, _arguments):
                return {"ok": False, "error": "canvas remains outside the viewport"}

        with self.assertRaisesRegex(BrowserRuntimeError, "not fully visible"):
            ensure_canvas_visible(Client(), "viewer-axial")

    def test_browser_heap_sample_records_unavailable_surface(self):
        trace = Trace(BrowserEngine.FIREFOX, "http://127.0.0.1/", "canvas", "0" * 40, {})
        measurement = browser_heap_sample(UnavailableHeapDriver(), trace, "initial")
        self.assertEqual(
            measurement,
            {"label": "initial", "available": False, "reason": "performance.memory unavailable"},
        )
        self.assertEqual(trace.metrics["browser_heap"], [measurement])

    def test_browser_heap_sample_rejects_invalid_ordering(self):
        trace = Trace(BrowserEngine.CHROMIUM, "http://127.0.0.1/", "canvas", "0" * 40, {})
        with self.assertRaisesRegex(BrowserRuntimeError, "heap ordering"):
            browser_heap_sample(InvalidHeapDriver(), trace, "initial")

    def test_browser_memory_sample_records_bounded_estimate(self):
        trace = Trace(BrowserEngine.CHROMIUM, "http://127.0.0.1/", "canvas", "0" * 40, {})
        measurement = browser_memory_sample(FakeDriver(), trace, "initial")
        self.assertEqual(
            measurement,
            {
                "label": "initial",
                "available": True,
                "source": "performance.measureUserAgentSpecificMemory",
                "secure_context": True,
                "cross_origin_isolated": True,
                "estimated_bytes": 8_388_608,
            },
        )
        self.assertEqual(trace.metrics["browser_memory"], [measurement])

    def test_browser_memory_sample_records_unavailable_surface(self):
        trace = Trace(BrowserEngine.FIREFOX, "http://127.0.0.1/", "canvas", "0" * 40, {})
        measurement = browser_memory_sample(UnavailableMemoryDriver(), trace, "initial")
        self.assertEqual(
            measurement,
            {"label": "initial", "available": False, "reason": "cross-origin isolation required"},
        )
        self.assertEqual(trace.metrics["browser_memory"], [measurement])

    def test_browser_memory_sample_records_failed_observations(self):
        for reason in ("memory measurement rejected", "memory measurement timed out"):
            with self.subTest(reason=reason):
                trace = Trace(BrowserEngine.CHROMIUM, "http://127.0.0.1/", "canvas", "0" * 40, {})
                measurement = browser_memory_sample(FailedMemoryDriver(reason), trace, "initial")
                self.assertEqual(measurement, {"label": "initial", "available": False, "reason": reason})
                self.assertEqual(trace.metrics["browser_memory"], [measurement])

    def test_browser_memory_sample_rejects_invalid_value(self):
        trace = Trace(BrowserEngine.CHROMIUM, "http://127.0.0.1/", "canvas", "0" * 40, {})
        with self.assertRaisesRegex(BrowserRuntimeError, "invalid estimated_bytes"):
            browser_memory_sample(InvalidMemoryDriver(), trace, "initial")

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
                ["viewer-axial"],
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

    def test_canvas_trace_rejects_untrusted_events(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            with self.assertRaisesRegex(BrowserRuntimeError, "was not trusted"):
                run_canvas_scenario(
                    UntrustedCanvasDriver(),
                    BrowserEngine.CHROMIUM,
                    "http://127.0.0.1:8080/ritk.html",
                    "0" * 40,
                    pathlib.Path(directory),
                    5_000,
                    ["viewer-axial"],
                    "1" * 40,
                )

    def test_canvas_trace_rejects_incomplete_pointer_events(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            with self.assertRaisesRegex(BrowserRuntimeError, "omitted"):
                run_canvas_scenario(
                    IncompleteCanvasDriver(),
                    BrowserEngine.CHROMIUM,
                    "http://127.0.0.1:8080/ritk.html",
                    "0" * 40,
                    pathlib.Path(directory),
                    5_000,
                    ["viewer-axial"],
                    "1" * 40,
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
                            ["viewer-axial"],
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

    def test_cli_rejects_keyboard_trace_on_fragment_scenario(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            trace_path = pathlib.Path(directory) / "invalid-keyboard-cli.json"
            with mock.patch.object(
                sys,
                "argv",
                [
                    "browser_runtime.py",
                    "--engine",
                    "chromium",
                    "--scenario",
                    "fragment",
                    "--keyboard-trace",
                    "--driver-url",
                    "http://127.0.0.1:9515",
                    "--url",
                    "http://127.0.0.1:8080/fragment.html",
                    "--output",
                    str(trace_path),
                ],
            ):
                self.assertEqual(main(), 1)
            document = json.loads(trace_path.read_text(encoding="utf-8"))
        self.assertEqual(document["status"], "failed")
        self.assertIn("requires --scenario canvas", document["error"])

    def test_cli_rejects_lifecycle_cycles_on_canvas_scenario(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            trace_path = pathlib.Path(directory) / "invalid-lifecycle-cli.json"
            with mock.patch.object(
                sys,
                "argv",
                [
                    "browser_runtime.py",
                    "--engine",
                    "chromium",
                    "--scenario",
                    "canvas",
                    "--lifecycle-cycles",
                    "2",
                    "--output",
                    str(trace_path),
                ],
            ):
                self.assertEqual(main(), 1)
            document = json.loads(trace_path.read_text(encoding="utf-8"))
        self.assertEqual(document["status"], "failed")
        self.assertIn("requires --scenario workbench", document["error"])

    def test_cli_rejects_accessibility_probe_on_canvas_scenario(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            trace_path = pathlib.Path(directory) / "invalid-accessibility-cli.json"
            with mock.patch.object(
                sys,
                "argv",
                [
                    "browser_runtime.py",
                    "--engine",
                    "chromium",
                    "--scenario",
                    "canvas",
                    "--accessibility-probe",
                    "--output",
                    str(trace_path),
                ],
            ):
                self.assertEqual(main(), 1)
            document = json.loads(trace_path.read_text(encoding="utf-8"))
        self.assertEqual(document["status"], "failed")
        self.assertIn("--accessibility-probe requires --scenario workbench", document["error"])

    def test_cli_rejects_asset_probe_on_canvas_scenario(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            trace_path = pathlib.Path(directory) / "invalid-asset-cli.json"
            with mock.patch.object(
                sys,
                "argv",
                [
                    "browser_runtime.py",
                    "--engine",
                    "chromium",
                    "--scenario",
                    "canvas",
                    "--asset-probe",
                    "--output",
                    str(trace_path),
                ],
            ):
                self.assertEqual(main(), 1)
            document = json.loads(trace_path.read_text(encoding="utf-8"))
        self.assertEqual(document["status"], "failed")
        self.assertIn("--asset-probe requires --scenario workbench", document["error"])

    def test_cli_rejects_media_probe_on_canvas_scenario(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            trace_path = pathlib.Path(directory) / "invalid-media-cli.json"
            with mock.patch.object(
                sys,
                "argv",
                [
                    "browser_runtime.py",
                    "--engine",
                    "chromium",
                    "--scenario",
                    "canvas",
                    "--media-probe",
                    "--output",
                    str(trace_path),
                ],
            ):
                self.assertEqual(main(), 1)
            document = json.loads(trace_path.read_text(encoding="utf-8"))
        self.assertEqual(document["status"], "failed")
        self.assertIn("--media-probe requires --scenario workbench", document["error"])

    def test_cli_rejects_media_playback_probe_on_canvas_scenario(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            trace_path = pathlib.Path(directory) / "invalid-media-playback-cli.json"
            with mock.patch.object(
                sys,
                "argv",
                [
                    "browser_runtime.py",
                    "--engine",
                    "chromium",
                    "--scenario",
                    "canvas",
                    "--media-playback-probe",
                    "--output",
                    str(trace_path),
                ],
            ):
                self.assertEqual(main(), 1)
            document = json.loads(trace_path.read_text(encoding="utf-8"))
        self.assertEqual(document["status"], "failed")
        self.assertIn("--media-playback-probe requires --scenario workbench", document["error"])

    def test_cli_rejects_font_load_probe_on_canvas_scenario(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            trace_path = pathlib.Path(directory) / "invalid-font-load-cli.json"
            with mock.patch.object(
                sys,
                "argv",
                [
                    "browser_runtime.py",
                    "--engine",
                    "chromium",
                    "--scenario",
                    "canvas",
                    "--font-load-probe",
                    "--output",
                    str(trace_path),
                ],
            ):
                self.assertEqual(main(), 1)
            document = json.loads(trace_path.read_text(encoding="utf-8"))
        self.assertEqual(document["status"], "failed")
        self.assertIn("--font-load-probe requires --scenario workbench", document["error"])

    def test_cli_rejects_text_geometry_probe_on_canvas_scenario(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            trace_path = pathlib.Path(directory) / "invalid-text-geometry-cli.json"
            with mock.patch.object(
                sys,
                "argv",
                [
                    "browser_runtime.py",
                    "--engine",
                    "chromium",
                    "--scenario",
                    "canvas",
                    "--text-geometry-probe",
                    "--output",
                    str(trace_path),
                ],
            ):
                self.assertEqual(main(), 1)
            document = json.loads(trace_path.read_text(encoding="utf-8"))
        self.assertEqual(document["status"], "failed")
        self.assertIn("--text-geometry-probe requires --scenario workbench", document["error"])

    def test_cli_rejects_media_requirement_without_accessibility_probe(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            trace_path = pathlib.Path(directory) / "invalid-media-cli.json"
            with mock.patch.object(
                sys,
                "argv",
                [
                    "browser_runtime.py",
                    "--engine",
                    "chromium",
                    "--require-reduced-motion",
                    "--output",
                    str(trace_path),
                ],
            ):
                self.assertEqual(main(), 1)
            document = json.loads(trace_path.read_text(encoding="utf-8"))
        self.assertEqual(document["status"], "failed")
        self.assertIn("--require-reduced-motion requires --accessibility-probe", document["error"])

    def test_cli_rejects_keyboard_submit_without_accessibility_probe(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            trace_path = pathlib.Path(directory) / "invalid-keyboard-submit.json"
            with mock.patch.object(
                sys,
                "argv",
                [
                    "browser_runtime.py",
                    "--engine",
                    "chromium",
                    "--bridge",
                    "authorized",
                    "--keyboard-submit",
                    "--output",
                    str(trace_path),
                ],
            ):
                self.assertEqual(main(), 1)
            document = json.loads(trace_path.read_text(encoding="utf-8"))
        self.assertEqual(document["status"], "failed")
        self.assertIn("--keyboard-submit requires --accessibility-probe", document["error"])

    def test_cli_requires_a_fixed_url_for_fragment_scenario(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            trace_path = pathlib.Path(directory) / "invalid-fragment-cli.json"
            with mock.patch.object(
                sys,
                "argv",
                [
                    "browser_runtime.py",
                    "--engine",
                    "chromium",
                    "--scenario",
                    "fragment",
                    "--driver-url",
                    "http://127.0.0.1:9515",
                    "--serve-dir",
                    "output/browser",
                    "--output",
                    str(trace_path),
                ],
            ):
                self.assertEqual(main(), 1)
            document = json.loads(trace_path.read_text(encoding="utf-8"))
        self.assertEqual(document["status"], "failed")
        self.assertIn("require --url", document["error"])

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
                lifecycle_cycles=2,
            )
        self.assertTrue(driver.closed)
        self.assertIn({"action": "submit", "state": "pending"}, trace.actions)
        self.assertIn({"action": "cancel-stop-remount", "stale_result": False}, trace.actions)
        self.assertFalse(any(action.get("state") == "success" for action in trace.actions))
        self.assertFalse(driver.pending)
        self.assertFalse(driver.success)
        self.assertEqual(trace.cleanup["lifecycle_cycles"], 2)

    def test_canvas_trace_rejects_invalid_frame_timing(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            with self.assertRaisesRegex(BrowserRuntimeError, "invalid intervals"):
                run_canvas_scenario(
                    InvalidFrameTimingDriver(),
                    BrowserEngine.CHROMIUM,
                    "http://127.0.0.1:8080/ritk.html",
                    "0" * 40,
                    pathlib.Path(directory),
                    5_000,
                    ["viewer-axial"],
                    "1" * 40,
                )

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

    def test_remount_failure_is_observed(self):
        output = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser" / "runtime-test"
        output.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=output) as directory:
            driver = EmptyRemountDriver()
            with self.assertRaisesRegex(BrowserRuntimeError, "did not restore mounted controls"):
                run_scenario(
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

    def test_driver_selects_absolute_file_paths_through_w3c_input(self):
        client = WebDriverClient("http://127.0.0.1:9515", 1)
        client.session_id = "session"
        requests = []

        def record(method, path, payload=None):
            requests.append((method, path, payload))
            return None

        client._request = record
        paths = [pathlib.Path("C:/studies/one.dcm"), pathlib.Path("C:/studies/two.dcm")]
        client.send_file_paths("opaque/id", paths)
        self.assertEqual(requests[0][0:2], ("POST", "/session/session/element/opaque%2Fid/value"))
        expected = "\n".join(str(path) for path in paths)
        self.assertEqual(requests[0][2], {"text": expected, "value": list(expected)})

    def test_driver_rejects_unbounded_file_input_values(self):
        client = WebDriverClient("http://127.0.0.1:9515", 1)
        client.session_id = "session"
        client._request = lambda method, path, payload=None: None
        with self.assertRaisesRegex(BrowserRuntimeError, "absolute"):
            client.send_file_paths("input", [pathlib.Path("relative.dcm")])
        with self.assertRaisesRegex(BrowserRuntimeError, "path"):
            client.send_file_paths("input", [pathlib.Path("C:/" + "x" * MAX_FILE_PATH_BYTES + ".dcm")])
        with self.assertRaisesRegex(BrowserRuntimeError, "path count"):
            client.send_file_paths("input", [pathlib.Path("C:/x.dcm")] * (MAX_FILE_INPUT_PATHS + 1))
        long_path = pathlib.Path("C:/" + "x" * (MAX_FILE_INPUT_VALUE_BYTES // 2) + ".dcm")
        with self.assertRaisesRegex(BrowserRuntimeError, "path"):
            client.send_file_paths("input", [long_path, long_path])

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

    def test_driver_dispatches_named_keyboard_actions(self):
        client = WebDriverClient("http://127.0.0.1:9515", 1)
        client.session_id = "session"
        requests = []

        def record(method, path, payload=None):
            requests.append((method, path, payload))
            return None

        client._request = record
        client.key_press("ArrowDown")
        self.assertEqual(requests[0][0:2], ("POST", "/session/session/actions"))
        keyboard = requests[0][2]["actions"][0]
        self.assertEqual(keyboard["type"], "key")
        self.assertEqual(
            [action["type"] for action in keyboard["actions"]],
            ["keyDown", "keyUp"],
        )
        self.assertEqual(
            [action["value"] for action in keyboard["actions"]],
            ["\ue015", "\ue015"],
        )

    def test_driver_rejects_unbounded_keyboard_values(self):
        client = WebDriverClient("http://127.0.0.1:9515", 1)
        client.session_id = "session"
        client._request = lambda method, path, payload=None: None
        with self.assertRaisesRegex(BrowserRuntimeError, "non-empty"):
            client.key_press("")
        with self.assertRaisesRegex(BrowserRuntimeError, "named key or one printable"):
            client.key_press("ArrowDownAgain")
        with self.assertRaisesRegex(BrowserRuntimeError, "trace bound"):
            client.key_press("x" * 65)

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

    def test_static_server_publishes_only_a_valid_loopback_port(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "port"
            publish_port(path, "http://127.0.0.1:4321/")
            self.assertEqual(path.read_text(encoding="ascii"), "4321")
            with self.assertRaisesRegex(BrowserRuntimeError, "invalid loopback port"):
                publish_port(path, "http://127.0.0.1/")

    def test_static_server_serves_loopback_without_hostname_resolution(self):
        root = pathlib.Path(__file__).resolve().parents[2] / "output" / "browser"
        root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=root) as directory:
            path = pathlib.Path(directory)
            (path / "probe.txt").write_bytes(b"loopback startup")
            with mock.patch("socket.getfqdn", side_effect=AssertionError("startup performed DNS")):
                with StaticServer(path) as origin:
                    opener = browser_protocol.urllib.request.build_opener(
                        browser_protocol.urllib.request.ProxyHandler({})
                    )
                    with opener.open(origin + "probe.txt", timeout=2) as response:
                        self.assertEqual(response.status, 200)
                        self.assertEqual(response.read(), b"loopback startup")


if __name__ == "__main__":
    unittest.main()
