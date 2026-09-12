"""Run format-neutral trusted input and visual traces for HTML5 canvases."""
from __future__ import annotations

import hashlib
import math
import pathlib
import re
import struct
from typing import Any, Mapping, Optional, Sequence, Tuple

from browser_protocol import ROOT, WebDriverClient, BrowserRuntimeError, _safe_path
from browser_trace import BrowserEngine, Trace, screenshot


CANVAS_ID_PATTERN = re.compile(r"[A-Za-z][A-Za-z0-9_-]{0,127}")
CANVAS_ATTRIBUTE_PATTERN = re.compile(r"data-[a-z][a-z0-9_.:-]{0,122}")
CANVAS_DRAG_START = (24, 24)
CANVAS_DRAG_END = (64, 48)
CANVAS_WHEEL_POSITION = (64, 48)
CANVAS_WHEEL_DELTA = (0, 120)
CANVAS_FRAME_SETTLE_COUNT = 2
MAX_CANVAS_DIMENSION = 4096
MAX_CANVAS_CSS_SIZE = 16_384.0
MAX_CANVAS_POSITION = 16_384.0
MAX_CANVAS_ATTRIBUTES = 16
MAX_CANVAS_ATTRIBUTE_VALUE_BYTES = 1024
MAX_CANVAS_EVENT_RECORDS = 32
CANVAS_EVENT_TYPES = ("pointerdown", "pointermove", "pointerup", "wheel")

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

CANVAS_FRAME_SETTLE_SCRIPT = """
const done = arguments[arguments.length - 1];
let remaining = arguments[0];
const settle = () => {
  if (remaining === 0) { done({ok: true}); return; }
  remaining -= 1;
  window.requestAnimationFrame(settle);
};
if (typeof window.requestAnimationFrame !== 'function') {
  done({ok: false});
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
) -> None:
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
    result = client.execute_async(CANVAS_FRAME_SETTLE_SCRIPT, [CANVAS_FRAME_SETTLE_COUNT])
    if not isinstance(result, dict) or result.get("ok") is not True:
        raise BrowserRuntimeError("browser did not expose requestAnimationFrame for canvas settling")


def _install_event_trace(client: WebDriverClient, canvas_ids: Sequence[str]) -> None:
    """Install one bounded, capture-phase observer for browser trust evidence."""
    result = client.execute(
        CANVAS_EVENT_INSTALL_SCRIPT,
        [list(canvas_ids), list(CANVAS_EVENT_TYPES), MAX_CANVAS_EVENT_RECORDS],
    )
    if not isinstance(result, dict) or result.get("ok") is not True:
        detail = result.get("error") if isinstance(result, dict) else result
        raise BrowserRuntimeError(f"browser event trace could not be installed: {detail!r}")
    expected_listener_count = len(canvas_ids) * len(CANVAS_EVENT_TYPES)
    if result.get("listener_count") != expected_listener_count:
        raise BrowserRuntimeError("browser event trace installed an unexpected listener count")


def _read_event_evidence(
    client: WebDriverClient,
    canvas_id: str,
    expected_types: Sequence[str],
) -> list[dict[str, Any]]:
    """Read and validate one consumed batch of trusted browser events."""
    result = client.execute(CANVAS_EVENT_READ_SCRIPT, [canvas_id])
    if not isinstance(result, dict) or not isinstance(result.get("events"), list):
        raise BrowserRuntimeError(f"browser event trace for {canvas_id!r} is malformed")
    if result.get("overflow") is not False:
        raise BrowserRuntimeError(f"browser event trace for {canvas_id!r} exceeded its bound")
    events = result["events"]
    if not events:
        raise BrowserRuntimeError(f"browser emitted no events for {canvas_id!r}")
    expected = set(expected_types)
    for event in events:
        if not isinstance(event, dict):
            raise BrowserRuntimeError(f"browser event trace for {canvas_id!r} contains a non-object")
        _validate_event_record(event, canvas_id, expected)
    observed_types = {event["type"] for event in events}
    missing_types = expected - observed_types
    if missing_types:
        raise BrowserRuntimeError(
            f"browser event trace for {canvas_id!r} omitted {sorted(missing_types)!r}"
        )
    return events


def _validate_event_record(event: Mapping[str, Any], canvas_id: str, expected_types: set[str]) -> None:
    """Require a bounded event record to report trusted delivery to its canvas."""
    event_type = event.get("type")
    if event_type not in CANVAS_EVENT_TYPES:
        raise BrowserRuntimeError(f"browser event trace for {canvas_id!r} has an unsupported type")
    if event_type not in expected_types:
        raise BrowserRuntimeError(f"browser event trace for {canvas_id!r} mixed input types")
    if event.get("is_trusted") is not True:
        raise BrowserRuntimeError(f"browser event {event_type!r} for {canvas_id!r} was not trusted")
    if event.get("target_id") != canvas_id:
        raise BrowserRuntimeError(f"browser event {event_type!r} targeted the wrong canvas")
    for name in ("client_x", "client_y", "delta_x", "delta_y"):
        value = event.get(name)
        if value is not None and (
            not isinstance(value, (int, float)) or not math.isfinite(float(value))
        ):
            raise BrowserRuntimeError(f"browser event {event_type!r} has an invalid {name}")


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
) -> None:
    """Capture one trusted canvas interaction on an open browser session."""
    canvas_ids = validate_canvas_ids(canvas_ids)
    canvas_attributes = validate_canvas_attributes(canvas_attributes)
    elements = {canvas_id: client.find(f"#{canvas_id}") for canvas_id in canvas_ids}
    actions_released = False
    event_trace_installed = False
    diagnostic_listener_count = 0
    try:
        _install_event_trace(client, canvas_ids)
        event_trace_installed = True
        screenshot(client, trace, screenshot_directory, "window-initial")
        for canvas_id in canvas_ids:
            element = elements[canvas_id]
            _canvas_snapshot(client, trace, canvas_id, f"{canvas_id}-initial", canvas_attributes)
            _element_screenshot(client, trace, screenshot_directory, f"{canvas_id}-initial", element)
            client.pointer_drag(element, CANVAS_DRAG_START, CANVAS_DRAG_END)
            trace.actions.append(
                {
                    "action": "trusted-pointer-drag",
                    "canvas": canvas_id,
                    "start": list(CANVAS_DRAG_START),
                    "end": list(CANVAS_DRAG_END),
                    "observed_events": _read_event_evidence(
                        client, canvas_id, ("pointerdown", "pointermove", "pointerup")
                    ),
                }
            )
            client.wheel(element, CANVAS_WHEEL_POSITION, CANVAS_WHEEL_DELTA)
            trace.actions.append(
                {
                    "action": "trusted-wheel",
                    "canvas": canvas_id,
                    "position": list(CANVAS_WHEEL_POSITION),
                    "delta": list(CANVAS_WHEEL_DELTA),
                    "observed_events": _read_event_evidence(client, canvas_id, ("wheel",)),
                }
            )
            settle_canvas_input(client)
            _canvas_snapshot(client, trace, canvas_id, f"{canvas_id}-after-input", canvas_attributes)
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
        if event_trace_installed:
            _cleanup_event_trace(client)
        if not actions_released:
            client.release_actions()


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
) -> Trace:
    """Exercise trusted pointer and wheel input for format-neutral canvases."""
    canvas_ids = validate_canvas_ids(canvas_ids)
    canvas_attributes = validate_canvas_attributes(canvas_attributes)
    trace: Optional[Trace] = None
    try:
        client.create_session(engine.webdriver_name)
        client.set_timeouts(timeout_ms)
        trace = Trace(engine, url, "canvas", revision, client.capabilities, consumer_revision)
        client.navigate(url)
        capture_canvas_trace(client, trace, screenshot_directory, canvas_ids, canvas_attributes)
        return trace
    finally:
        client.close()
