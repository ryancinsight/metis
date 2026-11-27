"""Verify the GPU-free Metis canvas input seam through real WebDriver actions."""
from __future__ import annotations

import argparse
import json
import pathlib
import urllib.parse
from typing import Any, Mapping, Sequence

from browser_canvas import ensure_canvas_visible, settle_canvas_input
from browser_canvas_capture import READ_SCREENSHOT_PIXELS, capture_screenshot, pixel_observation
from browser_protocol import ELEMENT_KEY, BrowserRuntimeError, StaticServer, WebDriverClient, parse_device_scale
from browser_runtime import _wait_for_text, _write_trace
from browser_trace import BrowserEngine, Trace, record_device_scale, screenshot as viewport_screenshot


CANVAS_ID = "canvas-input"
STATUS_ID = "canvas-input-status"


INSTALL_CANVAS_TRACE = """
const canvas = document.getElementById(arguments[0]);
if (!canvas) throw new Error(`canvas trace target is missing: ${arguments[0]}`);
if (window.canvasInputTrace) throw new Error('canvas trace is already installed');
const ownAdd = Object.getOwnPropertyDescriptor(canvas, 'addEventListener');
const ownRemove = Object.getOwnPropertyDescriptor(canvas, 'removeEventListener');
const add = canvas.addEventListener.bind(canvas);
const remove = canvas.removeEventListener.bind(canvas);
const entries = [], observations = [];
const capture = (options) => typeof options === 'boolean'
  ? options : Boolean(options && options.capture);
Object.defineProperty(canvas, 'addEventListener', {configurable: true, value(type, listener, options) {
  if (typeof listener !== 'function') return add(type, listener, options);
  const entry = {type, listener, capture: capture(options), active: true, wrapped: null};
  entry.wrapped = function(event) {
    const observation = {
      type, pointer_id: event.pointerId ?? null, button: event.button ?? null,
      buttons: event.buttons ?? null, trusted: event.isTrusted === true, returned: false,
      default_prevented: null, has_capture: null,
    };
    observations.push(observation);
    listener.call(this, event);
    Object.assign(observation, {returned: true, default_prevented: event.defaultPrevented === true,
      has_capture: Number.isInteger(event.pointerId) ? canvas.hasPointerCapture(event.pointerId) : null});
  };
  entries.push(entry);
  return add(type, entry.wrapped, options);
}});
Object.defineProperty(canvas, 'removeEventListener', {configurable: true, value(type, listener, options) {
  const registered = entries.find((entry) => entry.active && entry.type === type &&
    entry.listener === listener && entry.capture === capture(options));
  if (!registered) return remove(type, listener, options);
  const result = remove(type, registered.wrapped, options);
  registered.active = false;
  return result;
}});
const state = () => ({registrations: entries.length,
  active: entries.filter((entry) => entry.active).length,
  removals: entries.filter((entry) => !entry.active).length,
  observations: observations.slice()});
window.canvasInputTrace = {state, restore: () => {
  if (ownAdd) Object.defineProperty(canvas, 'addEventListener', ownAdd);
  else delete canvas.addEventListener;
  if (ownRemove) Object.defineProperty(canvas, 'removeEventListener', ownRemove);
  else delete canvas.removeEventListener;
  delete window.canvasInputTrace;
}};
return state();
"""


RASTER_DIAGNOSTIC = """
const canvas = document.getElementById(arguments[0]);
const read = (context) => {
  const result = {lost: 'unsupported'};
  try {
    if (!context) return {error: '2D context unavailable', lost: 'unavailable'};
    if (typeof context.isContextLost === 'function') result.lost = context.isContextLost();
    result.pixel = Array.from(context.getImageData(0, 0, 1, 1).data);
  } catch (error) { result.error = String(error).slice(0, 256); }
  return result;
};
const rect = canvas.getBoundingClientRect();
const style = getComputedStyle(canvas);
const plain = document.createElement('canvas');
plain.width = 100; plain.height = 80;
const context = plain.getContext('2d');
let control;
try {
  const pixels = new Uint8ClampedArray(100 * 80 * 4);
  for (let offset = 0; offset < pixels.length; offset += 4)
    pixels.set([17, 34, 51, 255], offset);
  context.putImageData(new ImageData(pixels, 100, 80), 0, 0);
  control = read(context);
} catch (error) { control = {error: String(error).slice(0, 256)}; }
return {source: read(canvas.getContext('2d')), plain: control,
  css: {left: rect.left, top: rect.top, width: rect.width, height: rect.height},
  backing: {width: canvas.width, height: canvas.height},
  visibility: document.visibilityState, device_pixel_ratio: window.devicePixelRatio,
  style: {display: style.display, visibility: style.visibility, opacity: style.opacity}};
"""


def _batch(client: WebDriverClient) -> list[list[Any]]:
    text = client.execute(
        "const e=document.getElementById(arguments[0]);return e?(e.textContent||'').trim():null;",
        [STATUS_ID],
    )
    if not isinstance(text, str) or len(text.encode("utf-8")) > 8_192:
        raise BrowserRuntimeError("canvas input fixture returned an invalid status")
    try:
        payload = json.loads(text)
    except json.JSONDecodeError as error:
        raise BrowserRuntimeError("canvas input result is not valid JSON") from error
    rows = payload.get("events") if isinstance(payload, dict) else None
    if not isinstance(rows, list) or not 1 <= len(rows) <= 32:
        raise BrowserRuntimeError("canvas input result has an invalid event count")
    if any(not isinstance(row, list) or len(row) != 11 for row in rows):
        raise BrowserRuntimeError("canvas input result record has an invalid field count")
    return rows


def _pointer_row(phase: str, pointer_id: int, x: float, y: float, button: int, buttons: int) -> list[Any]:
    return [phase, pointer_id, x, y, button, buttons, False, False, False, False, True]


def _validate_drag(records: Sequence[list[Any]]) -> list[list[Any]]:
    down = next((index for index, record in enumerate(records) if record[0] == "Down"), None)
    if down is None:
        raise BrowserRuntimeError("canvas input result omitted pointer down")
    gesture = list(records[down:])
    if len(gesture) != 3:
        raise BrowserRuntimeError("canvas input result did not contain one complete drag")
    pointer_id = gesture[0][1]
    expected = [_pointer_row("Down", pointer_id, 20.0, 30.0, 0, 1),
                _pointer_row("Move", pointer_id, 30.0, 30.0, -1, 1),
                _pointer_row("Up", pointer_id, 30.0, 30.0, 0, 0)]
    if gesture != expected:
        raise BrowserRuntimeError(f"Rust pointer records differ from their oracle: {gesture!r}")
    return gesture


def _trace(client: WebDriverClient) -> dict[str, Any]:
    state = client.execute(
        "return window.canvasInputTrace ? window.canvasInputTrace.state() : null;"
    )
    counts = ("registrations", "active", "removals")
    if not isinstance(state, dict) or any(type(state.get(field)) is not int for field in counts) \
            or not isinstance(state.get("observations"), list):
        raise BrowserRuntimeError("canvas input instrumentation returned invalid state")
    return state


def _validate_callbacks(observations: Sequence[Mapping[str, Any]], pointer_id: int) -> None:
    down = next((i for i, event in enumerate(observations) if event.get("type") == "pointerdown"), None)
    if down is None:
        raise BrowserRuntimeError("real canvas callback trace omitted pointer down")
    common = {"pointer_id": pointer_id, "trusted": True, "returned": True, "default_prevented": True}
    expected = [{**common, "type": event_type, "button": button, "buttons": buttons,
                 "has_capture": capture} for event_type, button, buttons, capture in zip(
                     ("pointerdown", "pointermove", "pointerup"), (0, -1, 0), (1, 1, 0),
                     (True, True, False), strict=True)]
    if list(observations[down : down + 3]) != expected:
        raise BrowserRuntimeError("real canvas callbacks differ from their capture oracle")


def _pointer_actions(actions: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [{"type": "pointer", "id": "metis-active-drop",
             "parameters": {"pointerType": "mouse"}, "actions": actions}]


def _screenshot_dimensions(scale_milli: int) -> tuple[int, int]:
    # CSS extents times physical pixels per CSS pixel; this fixture requires
    # integral boundaries so the screenshot contains no partial edge pixel.
    dimensions = tuple(divmod(extent * scale_milli, 1_000) for extent in (100, 80))
    if any(remainder for _, remainder in dimensions):
        raise BrowserRuntimeError("canvas fixture has a fractional physical pixel boundary")
    return tuple(extent for extent, _ in dimensions)


def run(
    webdriver: str, engine: BrowserEngine, serve_dir: pathlib.Path, timeout_seconds: float,
    device_scale_milli: int | None, revision: str,
) -> dict[str, Any]:
    if not 0 < timeout_seconds <= 120:
        raise BrowserRuntimeError("timeout-seconds must be greater than zero and at most 120")
    if len(revision) != 40 or any(character not in "0123456789abcdef" for character in revision):
        raise BrowserRuntimeError("revision must be one lowercase hexadecimal Git object ID")
    client = WebDriverClient(webdriver, timeout_seconds)
    trace_installed = False
    phase = "create browser session"
    try:
        with StaticServer(serve_dir) as origin:
            serve_dir = serve_dir.resolve()
            url = urllib.parse.urljoin(origin, "canvas-input.html")
            client.create_session(engine.resolve_webdriver_name(None), device_scale_milli)
            client.set_timeouts(int(timeout_seconds * 1_000))
            client.set_window_rect(640, 480)
            client.navigate(url)
            _wait_for_text(client, STATUS_ID, "loaded", include=False, timeout_ms=10_000)
            installed = client.execute(INSTALL_CANVAS_TRACE, [CANVAS_ID])
            trace_installed = True
            empty_trace = {"registrations": 0, "active": 0, "removals": 0,
                           "observations": []}
            if installed != empty_trace or _trace(client) != empty_trace:
                raise BrowserRuntimeError("canvas input instrumentation was not empty before start")
            phase = "start raster surface"
            client.click(client.find("#canvas-start"))
            _wait_for_text(client, STATUS_ID, "ready", include=False, timeout_ms=10_000)
            phase = "read rendered raster pixel"
            canvas = client.execute(
                "const c=document.getElementById(arguments[0]),r=c.getBoundingClientRect();"
                "return {width:c.width,height:c.height,css_width:r.width,css_height:r.height};", [CANVAS_ID],
            )
            expected_canvas = {"width": 100, "height": 80, "css_width": 100,
                               "css_height": 80}
            if canvas != expected_canvas:
                raise BrowserRuntimeError(f"raster canvas differs from its oracle: {canvas!r}")
            trace = Trace(engine, url, "none", revision, client.capabilities)
            scale = record_device_scale(client, trace, device_scale_milli)
            dimensions = _screenshot_dimensions(scale["effective_milli"])
            screenshot_dir = serve_dir / "runtime" / "screenshots" / engine.value
            capture_screenshot(
                client, trace, screenshot_dir, CANVAS_ID, expected_canvas, "2d",
            )
            screenshot = trace.screenshots[-1]
            if (screenshot["width"], screenshot["height"]) != dimensions:
                raise BrowserRuntimeError(f"canvas screenshot differs from physical extent {dimensions}")
            image_path = (screenshot_dir / f"{CANVAS_ID}.png").relative_to(serve_dir)
            decoded = client.execute_async(
                READ_SCREENSHOT_PIXELS, [image_path.as_posix(), 10_000],
            )
            try:
                observed_pixels = pixel_observation(
                    decoded, dimensions, bytes((17, 34, 51, 255)) * (dimensions[0] * dimensions[1]),
                )
            except BrowserRuntimeError as error:
                diagnostic = {}
                try:
                    viewport_screenshot(client, trace, screenshot_dir, "canvas-input-viewport")
                    diagnostic["viewport"] = trace.screenshots[-1]
                except (BrowserRuntimeError, OSError) as capture_error:
                    diagnostic["viewport_error"] = str(capture_error)[:512]
                try:
                    diagnostic["readback"] = client.execute(RASTER_DIAGNOSTIC, [CANVAS_ID])
                except BrowserRuntimeError as readback_error:
                    diagnostic["readback_error"] = str(readback_error)[:512]
                detail = json.dumps(diagnostic, sort_keys=True)[:2_048]
                raise BrowserRuntimeError(f"{error}; raster diagnostic: {detail}") from error
            canvas["pixel"] = decoded["pixels"][:4]
            registered = _trace(client)
            listeners = registered["registrations"]
            if listeners <= 0 or registered != {
                "registrations": listeners, "active": listeners, "removals": 0, "observations": [],
            }:
                raise BrowserRuntimeError(f"canvas DOM registration state is invalid: {registered!r}")
            phase = "complete ordinary pointer drag"
            ensure_canvas_visible(client, CANVAS_ID)
            element = client.find(f"#{CANVAS_ID}")
            client.pointer_drag(element, (-30, -10), (-20, -10), source_id="metis-active-drop")
            settle_canvas_input(client)
            client.click(client.find("#canvas-drain"))
            gesture = _validate_drag(_batch(client))
            completed = _trace(client)
            _validate_callbacks(completed["observations"], gesture[0][1])
            phase = "press and retain active pointer"
            client.perform_actions(_pointer_actions([
                {"type": "pointerMove", "origin": {ELEMENT_KEY: element}, "x": -30, "y": -10, "duration": 0},
                {"type": "pointerDown", "button": 0},
            ]))
            settle_canvas_input(client)
            active = _trace(client)
            active_down = active["observations"][-1] if active["observations"] else {}
            pointer_id = active_down.get("pointer_id")
            expected_down = {"type": "pointerdown", "pointer_id": pointer_id, "button": 0,
                             "buttons": 1, "trusted": True, "returned": True, "default_prevented": True,
                             "has_capture": True}
            if type(pointer_id) is not int or active_down != expected_down:
                raise BrowserRuntimeError(f"active pointer callback differs: {active_down!r}")
            phase = "drop surface while pointer is held"
            client.execute("document.getElementById('canvas-stop').click();")
            stopped = _trace(client)
            if stopped["registrations"] != listeners or stopped["removals"] != listeners or stopped["active"] != 0:
                raise BrowserRuntimeError(f"surface teardown did not remove each DOM listener: {stopped!r}")
            phase = "check capture after surface drop"
            capture_after_drop = client.execute(
                "return document.getElementById(arguments[0]).hasPointerCapture(arguments[1]);",
                [CANVAS_ID, pointer_id],
            )
            if capture_after_drop is not False:
                raise BrowserRuntimeError("surface drop retained active pointer capture")
            phase = "send input after listener removal"
            callback_count = len(stopped["observations"])
            client.perform_actions(_pointer_actions([
                {"type": "pointerMove", "origin": "pointer", "x": 10, "y": 0, "duration": 0},
                {"type": "pointerUp", "button": 0},
            ]))
            after_input = _trace(client)
            if len(after_input["observations"]) != callback_count:
                raise BrowserRuntimeError("removed canvas callback received pointer input after teardown")
            client.execute("window.canvasInputTrace.restore();")
            trace_installed = False
            return {
                "schema": 1, "status": "passed", "revision": revision, "engine": engine.value,
                "url": url, "capabilities": dict(client.capabilities), "canvas": canvas,
                "events": gesture, "callbacks": completed["observations"],
                "screenshot": screenshot,
                "device_scale": scale,
                "visible_pixels": {**observed_pixels, "method": "element-screenshot",
                                   "count": dimensions[0] * dimensions[1]},
                "cleanup": {
                    "surface_listeners": listeners, "registrations": stopped["registrations"],
                    "removals": stopped["removals"], "active_listeners": stopped["active"],
                    "active_pointer_id": pointer_id, "capture_after_drop": capture_after_drop,
                    "callback_count_after_drop": callback_count,
                    "callback_count_after_input": len(after_input["observations"]),
                },
            }
    except BrowserRuntimeError as error:
        raise BrowserRuntimeError(f"canvas input {phase}: {error}") from error
    finally:
        if trace_installed and client.session_id is not None:
            client.execute("if (window.canvasInputTrace) window.canvasInputTrace.restore();")
        if client.session_id is not None:
            client.release_actions()
        client.close()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", required=True, choices=[engine.value for engine in BrowserEngine])
    parser.add_argument("--driver-url", required=True)
    parser.add_argument("--serve-dir", type=pathlib.Path, required=True)
    parser.add_argument("--device-scale")
    parser.add_argument("--revision", required=True)
    parser.add_argument("--timeout-seconds", type=float, default=30.0)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    arguments = parser.parse_args()
    try:
        scale = parse_device_scale(arguments.device_scale) if arguments.device_scale else None
        document = run(arguments.driver_url, BrowserEngine.parse(arguments.engine),
                       arguments.serve_dir, arguments.timeout_seconds, scale, arguments.revision)
    except (BrowserRuntimeError, OSError, ValueError) as error:
        _write_trace(arguments.output, {"schema": 1, "status": "failed", "error": str(error)})
        return 1
    _write_trace(arguments.output, document)
    print(json.dumps(document, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
