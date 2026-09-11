"""Run format-neutral trusted input and visual traces for HTML5 canvases."""
from __future__ import annotations

import hashlib
import math
import pathlib
import re
import struct
from typing import Optional, Sequence, Tuple

from browser_protocol import ROOT, WebDriverClient, BrowserRuntimeError, _safe_path
from browser_trace import BrowserEngine, Trace, screenshot


CANVAS_ID_PATTERN = re.compile(r"[A-Za-z][A-Za-z0-9_-]{0,127}")
CANVAS_DRAG_START = (24, 24)
CANVAS_DRAG_END = (64, 48)
CANVAS_WHEEL_POSITION = (64, 48)
CANVAS_WHEEL_DELTA = (0, 120)
MAX_CANVAS_DIMENSION = 4096
MAX_CANVAS_CSS_SIZE = 16_384.0
MAX_CANVAS_POSITION = 16_384.0

CANVAS_SNAPSHOT_SCRIPT = """
const id = arguments[0];
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
};
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


def validate_consumer_revision(value: Optional[str]) -> Optional[str]:
    """Validate an optional consumer revision carried in a cross-repo trace."""
    if value is None:
        return None
    if not isinstance(value, str):
        raise BrowserRuntimeError("consumer revision must be a 40-hex Git revision")
    if not re.fullmatch(r"[0-9a-fA-F]{40}", value):
        raise BrowserRuntimeError("consumer revision must be a 40-hex Git revision")
    return value.lower()


def _canvas_snapshot(client: WebDriverClient, trace: Trace, canvas_id: str, label: str) -> None:
    """Record one canvas's DOM dimensions and CSS placement."""
    value = client.execute(CANVAS_SNAPSHOT_SCRIPT, [canvas_id])
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


def run_canvas_scenario(
    client: WebDriverClient,
    engine: BrowserEngine,
    url: str,
    revision: str,
    screenshot_directory: pathlib.Path,
    timeout_ms: int,
    canvas_ids: Sequence[str],
    consumer_revision: Optional[str] = None,
) -> Trace:
    """Exercise trusted pointer and wheel input for format-neutral canvases."""
    canvas_ids = validate_canvas_ids(canvas_ids)
    trace: Optional[Trace] = None
    actions_released = False
    try:
        client.create_session(engine.webdriver_name)
        client.set_timeouts(timeout_ms)
        trace = Trace(engine, url, "canvas", revision, client.capabilities, consumer_revision)
        client.navigate(url)
        elements = {canvas_id: client.find(f"#{canvas_id}") for canvas_id in canvas_ids}
        screenshot(client, trace, screenshot_directory, "window-initial")
        for canvas_id in canvas_ids:
            element = elements[canvas_id]
            _canvas_snapshot(client, trace, canvas_id, f"{canvas_id}-initial")
            _element_screenshot(client, trace, screenshot_directory, f"{canvas_id}-initial", element)
            client.pointer_drag(element, CANVAS_DRAG_START, CANVAS_DRAG_END)
            trace.actions.append(
                {
                    "action": "trusted-pointer-drag",
                    "canvas": canvas_id,
                    "start": list(CANVAS_DRAG_START),
                    "end": list(CANVAS_DRAG_END),
                }
            )
            client.wheel(element, CANVAS_WHEEL_POSITION, CANVAS_WHEEL_DELTA)
            trace.actions.append(
                {
                    "action": "trusted-wheel",
                    "canvas": canvas_id,
                    "position": list(CANVAS_WHEEL_POSITION),
                    "delta": list(CANVAS_WHEEL_DELTA),
                }
            )
            _canvas_snapshot(client, trace, canvas_id, f"{canvas_id}-after-input")
            _element_screenshot(client, trace, screenshot_directory, f"{canvas_id}-after-input", element)
        client.release_actions()
        actions_released = True
        screenshot(client, trace, screenshot_directory, "window-final")
        trace.cleanup = {
            "session_closed": False,
            "active_input_sources_released": True,
            "canvas_count": len(canvas_ids),
            "provider_listener_count": "unavailable from WebDriver",
        }
        return trace
    finally:
        if client.session_id is not None and not actions_released:
            client.release_actions()
        client.close()
