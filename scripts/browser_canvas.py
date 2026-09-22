"""Run format-neutral trusted input and visual traces for HTML5 canvases."""
from __future__ import annotations

import enum
import hashlib
import math
import pathlib
import re
import struct
import sys
from typing import Any, Mapping, Optional, Sequence, Tuple

from browser_protocol import ROOT, WebDriverClient, BrowserRuntimeError, _safe_path
from browser_trace import (
    BrowserEngine,
    Trace,
    browser_heap_sample,
    browser_memory_sample,
    frame_timing,
    record_device_scale,
    screenshot,
)


CANVAS_ID_PATTERN = re.compile(r"[A-Za-z][A-Za-z0-9_-]{0,127}")
CANVAS_ATTRIBUTE_PATTERN = re.compile(r"data-[a-z][a-z0-9_.:-]{0,122}")
CANVAS_DRAG_START = (24, 24)
CANVAS_DRAG_END = (64, 48)
CANVAS_WHEEL_POSITION = (64, 48)
CANVAS_WHEEL_DELTA = (0, 120)
CANVAS_FRAME_SETTLE_COUNT = 2
CANVAS_FRAME_SETTLE_TIMEOUT_MS = 2_000
MAX_CANVAS_DIMENSION = 4096
MAX_CANVAS_CSS_SIZE = 16_384.0
MAX_CANVAS_POSITION = 16_384.0
MAX_CANVAS_ATTRIBUTES = 16
MAX_CANVAS_ATTRIBUTE_VALUE_BYTES = 1024
MAX_CANVAS_EVENT_RECORDS = 32
MAX_CANVAS_KEY_METADATA_BYTES = 64
CANVAS_EVENT_TYPES = ("pointerdown", "pointermove", "pointerup", "wheel")
CANVAS_KEY_EVENT_TYPES = ("keydown", "keyup")
CANVAS_KEYBOARD_SOURCE_ID = "metis-keyboard"
CHROMIUM_BROWSER_NAMES = ("chrome", "MicrosoftEdge", "msedge")
EDGE_BROWSER_NAMES = ("MicrosoftEdge", "msedge")


class KeyboardTraceKind(str, enum.Enum):
    """Keyboard action profiles supported by the generic canvas trace."""

    NAVIGATION = "navigation"
    CINE_RATE = "cine-rate"

    @property
    def key(self) -> str:
        """Return the browser ``KeyboardEvent.key`` value for this profile."""
        return {self.NAVIGATION: "ArrowDown", self.CINE_RATE: "="}[self]

    @property
    def code(self) -> str:
        """Return the browser ``KeyboardEvent.code`` value for this profile."""
        return {self.NAVIGATION: "ArrowDown", self.CINE_RATE: "Equal"}[self]

    @classmethod
    def parse(cls, value: str) -> "KeyboardTraceKind":
        """Parse a command-line profile name."""
        try:
            return cls(value)
        except ValueError as error:
            choices = ", ".join(item.value for item in cls)
            raise BrowserRuntimeError(f"keyboard trace kind must be one of: {choices}") from error


class _KeyboardTransition(enum.Enum):
    """One stateful W3C keyboard transition used by the cine-rate trace."""

    INITIAL_DOWN = "initial-down"
    REPEAT_AND_RELEASE = "repeat-and-release"

    @property
    def expected_repeat(self) -> bool:
        """Return the repeat value expected on the transition's keydown event."""
        return self is self.REPEAT_AND_RELEASE

    def actions(self, key: str) -> list[dict[str, str]]:
        """Build the W3C actions while leaving initial keys held across calls."""
        actions = [{"type": "keyDown", "value": key}]
        if self is self.REPEAT_AND_RELEASE:
            actions.append({"type": "keyUp", "value": key})
        return actions

    @property
    def event_types(self) -> tuple[str, ...]:
        """Return the browser event types required from this transition."""
        if self is self.REPEAT_AND_RELEASE:
            return CANVAS_KEY_EVENT_TYPES
        return ("keydown",)

CANVAS_SNAPSHOT_SCRIPT = """
const id = arguments[0];
const attributeNames = arguments[1];
const canvas = document.getElementById(id);
if (!canvas || canvas.tagName.toLowerCase() !== 'canvas') return null;
const rect = canvas.getBoundingClientRect();
return {
  id,
  width: canvas.width,
  height: canvas.height,
  css_width: rect.width,
  css_height: rect.height,
  left: rect.left,
  top: rect.top,
  attributes: Object.fromEntries(attributeNames.map((name) => [name, canvas.getAttribute(name)])),
};
"""

CANVAS_FOCUS_SCRIPT = """
const id = arguments[0];
const canvas = document.getElementById(id);
if (!canvas || canvas.tagName.toLowerCase() !== 'canvas') return {ok: false, active_id: null};
canvas.focus();
return {
  ok: document.activeElement === canvas,
  active_id: document.activeElement && typeof document.activeElement.id === 'string'
    ? document.activeElement.id
    : null,
};
"""

CANVAS_SCROLL_INTO_VIEW_SCRIPT = """
const id = arguments[0];
const canvas = document.getElementById(id);
if (!canvas || canvas.tagName.toLowerCase() !== 'canvas') {
  return {ok: false, error: `canvas ${id} was not found`};
}
canvas.scrollIntoView({block: 'center', inline: 'center'});
const rect = canvas.getBoundingClientRect();
const visible = rect.left >= 0 && rect.top >= 0 &&
  rect.right <= window.innerWidth && rect.bottom <= window.innerHeight;
return {
  ok: visible,
  error: visible ? null : `canvas ${id} remains outside the viewport`,
  left: rect.left,
  top: rect.top,
  right: rect.right,
  bottom: rect.bottom,
};
"""

CANVAS_FRAME_SETTLE_SCRIPT = """
const done = arguments[arguments.length - 1];
let remaining = arguments[0];
const timeoutMs = arguments[1];
let finished = false;
let deadline = null;
const finish = (result) => {
  if (finished) return;
  finished = true;
  if (deadline !== null) window.clearTimeout(deadline);
  done(result);
};
if (document.visibilityState !== 'visible') {
  finish({ok: false, error: 'document is not visible'});
  return;
}
deadline = window.setTimeout(
  () => finish({ok: false, error: 'frame settling deadline exceeded'}),
  timeoutMs,
);
const settle = () => {
  if (document.visibilityState !== 'visible') {
    finish({ok: false, error: 'document is not visible'});
    return;
  }
  if (remaining === 0) { finish({ok: true}); return; }
  remaining -= 1;
  window.requestAnimationFrame(settle);
};
if (typeof window.requestAnimationFrame !== 'function') {
  finish({ok: false, error: 'requestAnimationFrame is unavailable'});
} else {
  settle();
}
"""

CANVAS_EVENT_INSTALL_SCRIPT = """
const ids = arguments[0];
const eventTypes = arguments[1];
const maxEvents = arguments[2];
if (window.__metisCanvasTraceState) return {ok: false, error: "canvas event trace is already installed"};
const events = Object.fromEntries(ids.map((id) => [id, []]));
const overflow = Object.fromEntries(ids.map((id) => [id, false]));
const registrations = [];
const canvases = ids.map((id) => document.getElementById(id));
if (canvases.some((canvas) => !canvas || canvas.tagName.toLowerCase() !== "canvas")) {
  const missing = ids[canvases.findIndex((canvas) => !canvas || canvas.tagName.toLowerCase() !== "canvas")];
  return {ok: false, error: `canvas ${missing} was not found`};
}
for (const id of ids) {
  const canvas = document.getElementById(id);
  for (const type of eventTypes) {
    const listener = (event) => {
      const records = events[id];
      if (records.length >= maxEvents) {
        overflow[id] = true;
        return;
      }
      records.push({
        type: event.type,
        is_trusted: event.isTrusted === true,
        target_id: event.target && typeof event.target.id === "string" ? event.target.id : null,
        client_x: Number.isFinite(event.clientX) ? event.clientX : null,
        client_y: Number.isFinite(event.clientY) ? event.clientY : null,
        delta_x: Number.isFinite(event.deltaX) ? event.deltaX : null,
        delta_y: Number.isFinite(event.deltaY) ? event.deltaY : null,
        key: typeof event.key === "string" ? event.key : null,
        code: typeof event.code === "string" ? event.code : null,
        repeat: event.repeat === true,
        alt_key: event.altKey === true,
        ctrl_key: event.ctrlKey === true,
        meta_key: event.metaKey === true,
        shift_key: event.shiftKey === true,
      });
    };
    canvas.addEventListener(type, listener, {capture: true, passive: true});
    registrations.push({canvas, type, listener});
  }
}
window.__metisCanvasTraceState = {events, overflow, registrations};
return {ok: true, listener_count: registrations.length};
"""

CANVAS_EVENT_READ_SCRIPT = """
const state = window.__metisCanvasTraceState;
const id = arguments[0];
if (!state || !Object.prototype.hasOwnProperty.call(state.events, id)) return null;
const events = state.events[id];
state.events[id] = [];
return {events, overflow: state.overflow[id] === true};
"""

CANVAS_EVENT_CLEANUP_SCRIPT = """
const state = window.__metisCanvasTraceState;
if (!state) return {ok: true, listener_count: 0};
for (const registration of state.registrations) {
  registration.canvas.removeEventListener(registration.type, registration.listener, true);
}
const listenerCount = state.registrations.length;
delete window.__metisCanvasTraceState;
return {ok: true, listener_count: listenerCount};
"""


def validate_canvas_ids(canvas_ids: Sequence[str]) -> Tuple[str, ...]:
    """Validate a finite list of DOM canvas identifiers before selector use."""
    if (
        isinstance(canvas_ids, (str, bytes))
        or not isinstance(canvas_ids, Sequence)
        or not 1 <= len(canvas_ids) <= 8
    ):
        raise BrowserRuntimeError("canvas scenario requires between one and eight canvas identifiers")
    validated = []
    seen = set()
    for canvas_id in canvas_ids:
        if not isinstance(canvas_id, str) or not CANVAS_ID_PATTERN.fullmatch(canvas_id):
            raise BrowserRuntimeError(f"canvas identifier is not a bounded HTML id: {canvas_id!r}")
        if canvas_id in seen:
            raise BrowserRuntimeError(f"canvas identifier is repeated: {canvas_id!r}")
        seen.add(canvas_id)
        validated.append(canvas_id)
    return tuple(validated)


def validate_canvas_attributes(attribute_names: Sequence[str]) -> Tuple[str, ...]:
    """Validate consumer-selected DOM attributes for bounded canvas snapshots."""
    if (
        isinstance(attribute_names, (str, bytes))
        or not isinstance(attribute_names, Sequence)
        or len(attribute_names) > MAX_CANVAS_ATTRIBUTES
    ):
        raise BrowserRuntimeError(
            f"canvas attribute names must be a sequence of at most {MAX_CANVAS_ATTRIBUTES} values"
        )
    validated = []
    seen = set()
    for name in attribute_names:
        if not isinstance(name, str) or not CANVAS_ATTRIBUTE_PATTERN.fullmatch(name):
            raise BrowserRuntimeError(f"canvas attribute name is not a bounded HTML name: {name!r}")
        if name in seen:
            raise BrowserRuntimeError(f"canvas attribute name is repeated: {name!r}")
        seen.add(name)
        validated.append(name)
    return tuple(validated)


def validate_consumer_revision(value: Optional[str]) -> Optional[str]:
    """Validate an optional consumer revision carried in a cross-repo trace."""
    if value is None:
        return None
    if not isinstance(value, str):
        raise BrowserRuntimeError("consumer revision must be a 40-hex Git revision")
    if not re.fullmatch(r"[0-9a-fA-F]{40}", value):
        raise BrowserRuntimeError("consumer revision must be a 40-hex Git revision")
    return value.lower()


def _canvas_snapshot(
    client: WebDriverClient,
    trace: Trace,
    canvas_id: str,
    label: str,
    attribute_names: Sequence[str],
) -> Mapping[str, Any]:
    """Record one canvas's dimensions, placement and consumer-selected attributes."""
    value = client.execute(CANVAS_SNAPSHOT_SCRIPT, [canvas_id, list(attribute_names)])
    if not isinstance(value, dict) or value.get("id") != canvas_id:
        raise BrowserRuntimeError(f"canvas {canvas_id!r} was not found as an HTML canvas")
    for key in ("width", "height"):
        dimension = value.get(key)
        if not isinstance(dimension, int) or not 0 < dimension <= MAX_CANVAS_DIMENSION:
            raise BrowserRuntimeError(f"canvas {canvas_id!r} has an invalid {key}: {dimension!r}")
    for key in ("css_width", "css_height"):
        coordinate = value.get(key)
        if (
            not isinstance(coordinate, (int, float))
            or not math.isfinite(float(coordinate))
            or not 0.0 < float(coordinate) <= MAX_CANVAS_CSS_SIZE
        ):
            raise BrowserRuntimeError(f"canvas {canvas_id!r} has an invalid {key}: {coordinate!r}")
    for key in ("left", "top"):
        coordinate = value.get(key)
        if (
            not isinstance(coordinate, (int, float))
            or not math.isfinite(float(coordinate))
            or not -MAX_CANVAS_POSITION <= float(coordinate) <= MAX_CANVAS_POSITION
        ):
            raise BrowserRuntimeError(f"canvas {canvas_id!r} has an invalid {key}: {coordinate!r}")
    attributes = value.get("attributes")
    if not isinstance(attributes, dict) or set(attributes) != set(attribute_names):
        raise BrowserRuntimeError(f"canvas {canvas_id!r} returned an invalid attribute snapshot")
    for name in attribute_names:
        attribute_value = attributes[name]
        if attribute_value is not None and (
            not isinstance(attribute_value, str)
            or len(attribute_value.encode("utf-8")) > MAX_CANVAS_ATTRIBUTE_VALUE_BYTES
        ):
            raise BrowserRuntimeError(f"canvas {canvas_id!r} attribute {name!r} exceeds its value bound")
    trace.snapshots.append({"label": label, "canvas": value})
    return value


def _canvas_action_offsets(canvas: Mapping[str, Any]) -> tuple[tuple[int, int], tuple[int, int], tuple[int, int]]:
    """Clamp trusted input offsets to the visible bounds of one canvas."""
    css_width = float(canvas["css_width"])
    css_height = float(canvas["css_height"])

    def clamp(requested: int, size: float) -> int:
        extent = max(0, math.floor(size / 2.0) - 1)
        return max(-extent, min(extent, requested))

    def point(requested: tuple[int, int]) -> tuple[int, int]:
        return clamp(requested[0], css_width), clamp(requested[1], css_height)

    return point(CANVAS_DRAG_START), point(CANVAS_DRAG_END), point(CANVAS_WHEEL_POSITION)


def _element_screenshot(client: WebDriverClient, trace: Trace, directory: pathlib.Path, label: str, element_id: str) -> None:
    """Save one bounded element PNG and record its digest and dimensions."""
    _safe_path(directory, directory=ROOT / "output")
    content = client.element_screenshot(element_id)
    path = _safe_path(directory / f"{label}.png", directory=directory)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    width, height = struct.unpack(">II", content[16:24])
    trace.screenshots.append(
        {
            "label": label,
            "path": path.relative_to(ROOT).as_posix(),
            "sha256": hashlib.sha256(content).hexdigest(),
            "width": width,
            "height": height,
            "bytes": len(content),
            "scope": "element",
        }
    )


def settle_canvas_input(client: WebDriverClient) -> None:
    """Yield through two browser frames so queued canvas input is observable."""
    result = client.execute_async(
        CANVAS_FRAME_SETTLE_SCRIPT,
        [CANVAS_FRAME_SETTLE_COUNT, CANVAS_FRAME_SETTLE_TIMEOUT_MS],
    )
    if not isinstance(result, dict) or result.get("ok") is not True:
        detail = result.get("error") if isinstance(result, dict) else result
        raise BrowserRuntimeError(f"browser canvas frame settling failed: {detail!r}")


def _focus_canvas(client: WebDriverClient, canvas_id: str) -> Mapping[str, Any]:
    """Focus one canvas and require the browser to report it as active."""
    result = client.execute(CANVAS_FOCUS_SCRIPT, [canvas_id])
    if (
        not isinstance(result, dict)
        or result.get("ok") is not True
        or result.get("active_id") != canvas_id
    ):
        raise BrowserRuntimeError(f"canvas {canvas_id!r} did not accept keyboard focus")
    return result


def ensure_canvas_visible(client: WebDriverClient, canvas_id: str) -> None:
    """Scroll one canvas fully into view before a WebDriver element capture."""
    result = client.execute(CANVAS_SCROLL_INTO_VIEW_SCRIPT, [canvas_id])
    if not isinstance(result, dict) or result.get("ok") is not True:
        detail = result.get("error") if isinstance(result, dict) else result
        raise BrowserRuntimeError(f"canvas {canvas_id!r} is not fully visible: {detail!r}")


def _install_event_trace(
    client: WebDriverClient,
    canvas_ids: Sequence[str],
    keyboard_trace: KeyboardTraceKind | None,
) -> None:
    """Install one bounded, capture-phase observer for browser trust evidence."""
    event_types = CANVAS_EVENT_TYPES + (
        CANVAS_KEY_EVENT_TYPES if keyboard_trace is not None else ()
    )
    result = client.execute(
        CANVAS_EVENT_INSTALL_SCRIPT,
        [list(canvas_ids), list(event_types), MAX_CANVAS_EVENT_RECORDS],
    )
    if not isinstance(result, dict) or result.get("ok") is not True:
        detail = result.get("error") if isinstance(result, dict) else result
        raise BrowserRuntimeError(f"browser event trace could not be installed: {detail!r}")
    expected_listener_count = len(canvas_ids) * len(event_types)
    if result.get("listener_count") != expected_listener_count:
        raise BrowserRuntimeError("browser event trace installed an unexpected listener count")


def _read_event_evidence(
    client: WebDriverClient,
    canvas_id: str,
    expected_types: Sequence[str],
) -> list[dict[str, Any]]:
    """Read and validate one consumed batch of trusted browser events."""
    events = _consume_event_evidence(client, canvas_id)
    _validate_event_evidence(events, canvas_id, expected_types)
    return events


def _consume_event_evidence(
    client: WebDriverClient,
    canvas_id: str,
) -> list[dict[str, Any]]:
    """Consume one structurally bounded event batch before semantic checks."""
    result = client.execute(CANVAS_EVENT_READ_SCRIPT, [canvas_id])
    if not isinstance(result, dict) or not isinstance(result.get("events"), list):
        raise BrowserRuntimeError(f"browser event trace for {canvas_id!r} is malformed")
    if result.get("overflow") is not False:
        raise BrowserRuntimeError(f"browser event trace for {canvas_id!r} exceeded its bound")
    events = result["events"]
    if not events:
        raise BrowserRuntimeError(f"browser emitted no events for {canvas_id!r}")
    if len(events) > MAX_CANVAS_EVENT_RECORDS:
        raise BrowserRuntimeError(f"browser event trace for {canvas_id!r} exceeded its bound")
    for event in events:
        if not isinstance(event, dict):
            raise BrowserRuntimeError(f"browser event trace for {canvas_id!r} contains a non-object")
        _validate_event_record_shape(event, canvas_id)
    return events


def _validate_event_evidence(
    events: Sequence[Mapping[str, Any]],
    canvas_id: str,
    expected_types: Sequence[str],
) -> None:
    """Require a bounded event batch to report the requested trusted delivery."""
    expected = set(expected_types)
    for event in events:
        _validate_event_record(event, canvas_id, expected)
    observed_types = {event["type"] for event in events}
    missing_types = expected - observed_types
    if missing_types:
        raise BrowserRuntimeError(
            f"browser event trace for {canvas_id!r} omitted {sorted(missing_types)!r}"
        )


def _validate_event_record_shape(event: Mapping[str, Any], canvas_id: str) -> None:
    """Validate event fields that bound storage independently of semantics."""
    event_type = event.get("type")
    if event_type not in CANVAS_EVENT_TYPES + CANVAS_KEY_EVENT_TYPES:
        raise BrowserRuntimeError(f"browser event trace for {canvas_id!r} has an unsupported type")
    for name in ("client_x", "client_y", "delta_x", "delta_y"):
        value = event.get(name)
        if value is not None and (
            not isinstance(value, (int, float)) or not math.isfinite(float(value))
        ):
            raise BrowserRuntimeError(f"browser event {event_type!r} has an invalid {name}")
    if event_type in CANVAS_KEY_EVENT_TYPES:
        for name in ("key", "code"):
            value = event.get(name)
            if (
                not isinstance(value, str)
                or not value
                or len(value.encode("utf-8")) > MAX_CANVAS_KEY_METADATA_BYTES
            ):
                raise BrowserRuntimeError(f"browser event {event_type!r} has invalid {name} metadata")
        if type(event.get("repeat")) is not bool:
            raise BrowserRuntimeError(f"browser event {event_type!r} has invalid repeat metadata")
        for name in ("alt_key", "ctrl_key", "meta_key", "shift_key"):
            if type(event.get(name)) is not bool:
                raise BrowserRuntimeError(f"browser event {event_type!r} has invalid modifier metadata")


def _validate_event_record(event: Mapping[str, Any], canvas_id: str, expected_types: set[str]) -> None:
    """Require an event record to report trusted delivery to its canvas."""
    event_type = event.get("type")
    if event_type not in expected_types:
        raise BrowserRuntimeError(f"browser event trace for {canvas_id!r} mixed input types")
    if event.get("is_trusted") is not True:
        raise BrowserRuntimeError(f"browser event {event_type!r} for {canvas_id!r} was not trusted")
    if event.get("target_id") != canvas_id:
        raise BrowserRuntimeError(f"browser event {event_type!r} targeted the wrong canvas")


def _validate_keyboard_evidence(
    events: Sequence[Mapping[str, Any]],
    key: str,
    code: str,
    transition: _KeyboardTransition,
) -> None:
    """Require exact key identity and repeat state for one keyboard transition."""
    expected = [("keydown", transition.expected_repeat)]
    if transition is _KeyboardTransition.REPEAT_AND_RELEASE:
        expected.append(("keyup", False))
    observed = []
    for event in events:
        if event.get("key") != key or event.get("code") != code:
            raise BrowserRuntimeError("browser keyboard event reported unexpected key metadata")
        observed.append((event["type"], event["repeat"]))
    if observed != expected:
        raise BrowserRuntimeError(
            f"browser keyboard events reported {observed!r}; expected {expected!r}"
        )


def _capture_cine_rate_keyboard_trace(
    client: WebDriverClient,
    trace: Trace,
    screenshot_directory: pathlib.Path,
    canvas_id: str,
    element: str,
    canvas_attributes: Sequence[str],
) -> None:
    """Capture increase, decrease and held-key repeat evidence for cine rate."""
    steps = (
        ("=", "Equal", 187, _KeyboardTransition.INITIAL_DOWN, "after-keyboard"),
        ("=", "Equal", 187, _KeyboardTransition.REPEAT_AND_RELEASE, "after-repeat"),
        ("-", "Minus", 189, _KeyboardTransition.INITIAL_DOWN, "after-decrease"),
        ("-", "Minus", 189, _KeyboardTransition.REPEAT_AND_RELEASE, "after-decrease-repeat"),
    )
    devtools_endpoint = _chromium_devtools_endpoint(client)
    pending_devtools_key = None
    try:
        for key, code, virtual_key_code, transition, label_suffix in steps:
            focus = _focus_canvas(client, canvas_id)
            if devtools_endpoint is not None and transition is _KeyboardTransition.INITIAL_DOWN:
                pending_devtools_key = (key, code, virtual_key_code)
            transport = _dispatch_cine_rate_key(
                client,
                key,
                code,
                virtual_key_code,
                transition,
                devtools_endpoint,
            )
            if devtools_endpoint is not None and transition is _KeyboardTransition.REPEAT_AND_RELEASE:
                pending_devtools_key = None
            settle_canvas_input(client)
            observed_events = _consume_event_evidence(client, canvas_id)
            trace.actions.append(
                {
                    "action": "trusted-keyboard",
                    "canvas": canvas_id,
                    "key": key,
                    "code": code,
                    "repeat": transition.expected_repeat,
                    "transport": transport,
                    "focus": dict(focus),
                    "observed_events": observed_events,
                }
            )
            _validate_event_evidence(observed_events, canvas_id, transition.event_types)
            _validate_keyboard_evidence(observed_events, key, code, transition)
            label = f"{canvas_id}-{label_suffix}"
            _canvas_snapshot(client, trace, canvas_id, label, canvas_attributes)
            ensure_canvas_visible(client, canvas_id)
            _element_screenshot(client, trace, screenshot_directory, label, element)
    finally:
        if devtools_endpoint is not None and pending_devtools_key is not None:
            primary_error = sys.exc_info()[1]
            key, code, virtual_key_code = pending_devtools_key
            try:
                _dispatch_devtools_key_event(
                    client,
                    devtools_endpoint,
                    "keyUp",
                    key,
                    code,
                    virtual_key_code,
                    False,
                )
            except BrowserRuntimeError as cleanup_error:
                if primary_error is None:
                    raise
                primary_error.add_note(f"CDP key release also failed: {cleanup_error}")


def _dispatch_cine_rate_key(
    client: WebDriverClient,
    key: str,
    code: str,
    virtual_key_code: int,
    transition: _KeyboardTransition,
    devtools_endpoint: str | None,
) -> str:
    """Dispatch one cine-rate transition through the browser's explicit capability."""
    if devtools_endpoint is None:
        client.perform_actions(
            [
                {
                    "type": "key",
                    "id": CANVAS_KEYBOARD_SOURCE_ID,
                    "actions": transition.actions(key),
                }
            ]
        )
        return "webdriver-actions"

    events = [("keyDown", transition.expected_repeat)]
    if transition is _KeyboardTransition.REPEAT_AND_RELEASE:
        events.append(("keyUp", False))
    for event_type, auto_repeat in events:
        _dispatch_devtools_key_event(
            client,
            devtools_endpoint,
            event_type,
            key,
            code,
            virtual_key_code,
            auto_repeat,
        )
    return "chromium-devtools"


def _chromium_devtools_endpoint(client: WebDriverClient) -> str | None:
    """Select the vendor WebDriver endpoint for a reported Chromium browser."""
    browser_name = client.capabilities.get("browserName")
    if browser_name not in CHROMIUM_BROWSER_NAMES:
        return None
    return "ms/cdp/execute" if browser_name in EDGE_BROWSER_NAMES else "goog/cdp/execute"


def _dispatch_devtools_key_event(
    client: WebDriverClient,
    endpoint: str,
    event_type: str,
    key: str,
    code: str,
    virtual_key_code: int,
    auto_repeat: bool,
) -> None:
    """Send one fully specified Chromium key event through WebDriver's CDP endpoint."""
    client._request(
        "POST",
        client._session_path(endpoint),
        {
            "cmd": "Input.dispatchKeyEvent",
            "params": {
                "type": event_type,
                "key": key,
                "code": code,
                "windowsVirtualKeyCode": virtual_key_code,
                "autoRepeat": auto_repeat,
            },
        },
    )


def _cleanup_event_trace(client: WebDriverClient) -> int:
    """Remove the diagnostic listeners and require every registration to be released."""
    result = client.execute(CANVAS_EVENT_CLEANUP_SCRIPT)
    if not isinstance(result, dict) or result.get("ok") is not True:
        raise BrowserRuntimeError("browser event trace cleanup failed")
    if not isinstance(result.get("listener_count"), int) or result["listener_count"] <= 0:
        raise BrowserRuntimeError("browser event trace cleanup released no listeners")
    return result["listener_count"]


def capture_canvas_trace(
    client: WebDriverClient,
    trace: Trace,
    screenshot_directory: pathlib.Path,
    canvas_ids: Sequence[str],
    canvas_attributes: Sequence[str] = (),
    frame_timeout_ms: int = 4_000,
    browser_heap: bool = False,
    keyboard_trace: KeyboardTraceKind | None = None,
    browser_memory: bool = False,
) -> None:
    """Capture trusted pointer and wheel input, with optional keyboard evidence."""
    if keyboard_trace is not None and not isinstance(keyboard_trace, KeyboardTraceKind):
        raise BrowserRuntimeError("keyboard trace kind must be a KeyboardTraceKind or None")
    canvas_ids = validate_canvas_ids(canvas_ids)
    canvas_attributes = validate_canvas_attributes(canvas_attributes)
    elements = {canvas_id: client.find(f"#{canvas_id}") for canvas_id in canvas_ids}
    actions_released = False
    event_trace_installed = False
    diagnostic_listener_count = 0
    try:
        _install_event_trace(client, canvas_ids, keyboard_trace)
        event_trace_installed = True
        screenshot(client, trace, screenshot_directory, "window-initial")
        for canvas_id in canvas_ids:
            element = elements[canvas_id]
            initial_canvas = _canvas_snapshot(client, trace, canvas_id, f"{canvas_id}-initial", canvas_attributes)
            drag_start, drag_end, wheel_position = _canvas_action_offsets(initial_canvas)
            if browser_heap:
                browser_heap_sample(client, trace, f"{canvas_id}-initial")
            if browser_memory:
                browser_memory_sample(client, trace, f"{canvas_id}-initial")
            frame_timing(client, trace, f"{canvas_id}-initial", timeout_ms=frame_timeout_ms)
            ensure_canvas_visible(client, canvas_id)
            _element_screenshot(client, trace, screenshot_directory, f"{canvas_id}-initial", element)
            if keyboard_trace is KeyboardTraceKind.CINE_RATE:
                _capture_cine_rate_keyboard_trace(
                    client,
                    trace,
                    screenshot_directory,
                    canvas_id,
                    element,
                    canvas_attributes,
                )
            elif keyboard_trace is not None:
                focus = _focus_canvas(client, canvas_id)
                client.key_press(keyboard_trace.key)
                trace.actions.append(
                    {
                        "action": "trusted-keyboard",
                        "canvas": canvas_id,
                        "key": keyboard_trace.key,
                        "code": keyboard_trace.code,
                        "repeat": False,
                        "transport": "webdriver-actions",
                        "focus": dict(focus),
                        "observed_events": _read_event_evidence(
                            client, canvas_id, CANVAS_KEY_EVENT_TYPES
                        ),
                    }
                )
                settle_canvas_input(client)
                _canvas_snapshot(
                    client,
                    trace,
                    canvas_id,
                    f"{canvas_id}-after-keyboard",
                    canvas_attributes,
                )
            client.pointer_drag(element, drag_start, drag_end)
            trace.actions.append(
                {
                    "action": "trusted-pointer-drag",
                    "canvas": canvas_id,
                    "start": list(drag_start),
                    "end": list(drag_end),
                    "observed_events": _read_event_evidence(
                        client, canvas_id, ("pointerdown", "pointermove", "pointerup")
                    ),
                }
            )
            client.wheel(element, wheel_position, CANVAS_WHEEL_DELTA)
            trace.actions.append(
                {
                    "action": "trusted-wheel",
                    "canvas": canvas_id,
                    "position": list(wheel_position),
                    "delta": list(CANVAS_WHEEL_DELTA),
                    "observed_events": _read_event_evidence(client, canvas_id, ("wheel",)),
                }
            )
            settle_canvas_input(client)
            _canvas_snapshot(client, trace, canvas_id, f"{canvas_id}-after-input", canvas_attributes)
            if browser_heap:
                browser_heap_sample(client, trace, f"{canvas_id}-after-input")
            if browser_memory:
                browser_memory_sample(client, trace, f"{canvas_id}-after-input")
            frame_timing(client, trace, f"{canvas_id}-after-input", timeout_ms=frame_timeout_ms)
            ensure_canvas_visible(client, canvas_id)
            _element_screenshot(client, trace, screenshot_directory, f"{canvas_id}-after-input", element)
        client.release_actions()
        actions_released = True
        diagnostic_listener_count = _cleanup_event_trace(client)
        event_trace_installed = False
        screenshot(client, trace, screenshot_directory, "window-final")
        trace.cleanup = {
            "session_closed": False,
            "active_input_sources_released": True,
            "canvas_count": len(canvas_ids),
            "canvas_attribute_names": list(canvas_attributes),
            "provider_listener_count": "unavailable from WebDriver",
            "diagnostic_listener_count": diagnostic_listener_count,
            "diagnostic_listeners_released": True,
        }
    finally:
        primary_error = sys.exc_info()[1]
        cleanup_errors = []
        if event_trace_installed:
            try:
                _cleanup_event_trace(client)
            except BrowserRuntimeError as cleanup_error:
                cleanup_errors.append(("canvas event listener cleanup", cleanup_error))
        if not actions_released:
            try:
                client.release_actions()
            except BrowserRuntimeError as cleanup_error:
                cleanup_errors.append(("browser input release", cleanup_error))
        if cleanup_errors:
            if primary_error is not None:
                for operation, cleanup_error in cleanup_errors:
                    primary_error.add_note(f"{operation} also failed: {cleanup_error}")
            else:
                operation, cleanup_error = cleanup_errors[0]
                for later_operation, later_error in cleanup_errors[1:]:
                    cleanup_error.add_note(f"{later_operation} also failed: {later_error}")
                cleanup_error.add_note(f"failed operation: {operation}")
                raise cleanup_error


def run_canvas_scenario(
    client: WebDriverClient,
    engine: BrowserEngine,
    url: str,
    revision: str,
    screenshot_directory: pathlib.Path,
    timeout_ms: int,
    canvas_ids: Sequence[str],
    consumer_revision: Optional[str] = None,
    canvas_attributes: Sequence[str] = (),
    browser_heap: bool = False,
    keyboard_trace: KeyboardTraceKind | None = None,
    browser_name: Optional[str] = None,
    device_scale_milli: Optional[int] = None,
    browser_memory: bool = False,
) -> Trace:
    """Exercise trusted canvas input for format-neutral canvases."""
    canvas_ids = validate_canvas_ids(canvas_ids)
    canvas_attributes = validate_canvas_attributes(canvas_attributes)
    trace: Optional[Trace] = None
    try:
        client.create_session(engine.resolve_webdriver_name(browser_name), device_scale_milli)
        client.set_timeouts(timeout_ms)
        trace = Trace(engine, url, "canvas", revision, client.capabilities, consumer_revision)
        client.navigate(url)
        record_device_scale(client, trace, device_scale_milli)
        capture_canvas_trace(
            client,
            trace,
            screenshot_directory,
            canvas_ids,
            canvas_attributes,
            frame_timeout_ms=timeout_ms,
            browser_heap=browser_heap,
            keyboard_trace=keyboard_trace,
            browser_memory=browser_memory,
        )
        return trace
    finally:
        client.close()
