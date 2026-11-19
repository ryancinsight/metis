"""Canvas capture modes for format-neutral browser evidence."""
from __future__ import annotations

import enum
import hashlib
import pathlib
import re
from typing import Any, Mapping

from browser_canvas import _element_screenshot, ensure_canvas_visible, settle_canvas_input
from browser_protocol import BrowserRuntimeError, WebDriverClient
from browser_trace import Trace


CONTEXT_NAME_PATTERN = re.compile(r"[a-z0-9][a-z0-9-]{0,31}\Z")


CANVAS_CONTEXT = """
const id = arguments[0];
const contextName = arguments[1];
const canvas = document.getElementById(id);
if (!(canvas instanceof HTMLCanvasElement))
  return {diagnostic: 'canvas is missing'};
if (!Number.isInteger(canvas.width) || !Number.isInteger(canvas.height) ||
    canvas.width <= 0 || canvas.height <= 0 || canvas.width > 4096 || canvas.height > 4096)
  return {diagnostic: 'canvas dimensions are outside the capture bound'};
if (canvas.getContext(contextName) === null)
  return {diagnostic: `canvas context ${contextName} is unavailable`};
return {width: canvas.width, height: canvas.height, context: contextName};
"""


class CanvasCaptureMode(str, enum.Enum):
    """Capture representation supported by the generic browser runner."""

    RGBA = "rgba"
    SCREENSHOT = "screenshot"

    @classmethod
    def parse(cls, value: str) -> "CanvasCaptureMode":
        """Parse one explicit canvas capture mode."""
        try:
            return cls(value)
        except ValueError as error:
            choices = ", ".join(item.value for item in cls)
            raise BrowserRuntimeError(f"canvas capture mode must be one of: {choices}") from error


def validate_context_name(value: Any) -> str:
    """Validate the consumer-selected canvas context name."""
    if not isinstance(value, str) or not CONTEXT_NAME_PATTERN.fullmatch(value):
        raise BrowserRuntimeError("canvas context must be a bounded lowercase name")
    return value


def capture_screenshot(
    client: WebDriverClient,
    trace: Trace,
    directory: pathlib.Path,
    canvas_id: str,
    expected: Mapping[str, Any],
    context_name: str,
    screenshot_label: str | None = None,
) -> dict[str, Any]:
    """Capture one canvas through its element PNG and verify its context."""
    surface = client.execute(CANVAS_CONTEXT, [canvas_id, context_name])
    if not isinstance(surface, dict) or "diagnostic" in surface:
        detail = surface.get("diagnostic") if isinstance(surface, dict) else surface
        raise BrowserRuntimeError(f"canvas {canvas_id!r} cannot provide {context_name}: {detail!r}")
    if any(key not in expected for key in ("width", "height")):
        raise BrowserRuntimeError(
            f"canvas {canvas_id!r} screenshot oracle must include intrinsic dimensions"
        )
    wanted_surface = {key: expected[key] for key in ("width", "height")}
    if {key: surface[key] for key in wanted_surface} != wanted_surface:
        raise BrowserRuntimeError(
            f"canvas {canvas_id}: expected intrinsic dimensions {wanted_surface}, "
            f"found {surface}"
        )
    ensure_canvas_visible(client, canvas_id)
    settle_canvas_input(client)
    _element_screenshot(
        client,
        trace,
        directory,
        screenshot_label or canvas_id,
        client.find("#" + canvas_id),
    )
    screenshot = trace.screenshots[-1]
    return {
        **surface,
        "screenshot_width": screenshot["width"],
        "screenshot_height": screenshot["height"],
        "screenshot_bytes": screenshot["bytes"],
        "screenshot_sha256": screenshot["sha256"],
    }


def compare_screenshot_stability(
    client: WebDriverClient,
    trace: Trace,
    directory: pathlib.Path,
    canvas_ids: tuple[str, ...],
    observations: Mapping[str, Mapping[str, Any]],
    context_name: str,
) -> None:
    """Require rejected input to leave every screenshot unchanged."""
    for canvas_id in canvas_ids:
        expected = observations[canvas_id]
        actual = capture_screenshot(
            client,
            trace,
            directory,
            canvas_id,
            expected,
            context_name,
            screenshot_label=f"{canvas_id}-after-rejections",
        )
        if actual["screenshot_sha256"] != expected["screenshot_sha256"]:
            raise BrowserRuntimeError(
                f"a rejected file batch changed canvas {canvas_id} screenshot"
            )


READ_SCREENSHOT_PIXELS = """
const done = arguments[arguments.length - 1];
const path = arguments[0];
const limit = arguments[1];
let settled = false;
const finish = (value) => {
  if (settled) return;
  settled = true;
  window.clearTimeout(timer);
  done(value);
};
const timer = window.setTimeout(
  () => finish({ok: false, error: 'screenshot decode deadline exceeded'}), limit
);
const image = new Image();
image.src = new URL(path, document.baseURI).href;
image.decode().then(() => {
  const copy = document.createElement('canvas');
  copy.width = image.naturalWidth;
  copy.height = image.naturalHeight;
  const context = copy.getContext('2d', {willReadFrequently: true});
  if (!context) throw new Error('2D screenshot context is unavailable');
  context.drawImage(image, 0, 0);
  finish({
    ok: true, width: copy.width, height: copy.height,
    pixels: Array.from(context.getImageData(0, 0, copy.width, copy.height).data)
  });
}).catch((error) => finish({ok: false, error: String(error).slice(0, 512)}));
"""


def pixel_observation(
    value: Any, dimensions: tuple[int, int], expected_rgba: bytes,
) -> dict[str, Any]:
    """Compare every decoded pixel with an independently supplied frame."""
    if not isinstance(value, dict) or value.get("ok") is not True:
        detail = value.get("error") if isinstance(value, dict) else value
        raise BrowserRuntimeError(f"canvas pixel readback failed: {detail!r}")
    width, height = dimensions
    if value.get("width") != width or value.get("height") != height:
        raise BrowserRuntimeError(f"canvas dimensions differ from {dimensions}: {value!r}")
    pixels = value.get("pixels")
    expected_length = width * height * 4
    if len(expected_rgba) != expected_length:
        raise BrowserRuntimeError("pixel oracle length differs from its dimensions")
    if not isinstance(pixels, list) or len(pixels) != expected_length:
        raise BrowserRuntimeError("canvas returned an invalid RGBA buffer")
    raw = bytearray()
    for index, component in enumerate(pixels):
        if type(component) is not int or not 0 <= component <= 255:
            raise BrowserRuntimeError(f"RGBA component {index} is invalid: {component!r}")
        raw.append(component)
    for pixel_index in range(width * height):
        offset = pixel_index * 4
        expected = tuple(expected_rgba[offset:offset + 4])
        actual = tuple(raw[offset:offset + 4])
        if actual != expected:
            x, y = pixel_index % width, pixel_index // width
            raise BrowserRuntimeError(f"pixel ({x}, {y}) is {actual}, expected {expected}")
    return {
        "width": width, "height": height,
        "rgba_sha256": hashlib.sha256(raw).hexdigest(),
    }
