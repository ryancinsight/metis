"""Canvas capture modes for format-neutral browser evidence."""
from __future__ import annotations

import enum
import pathlib
import re
from typing import Any, Mapping

from browser_canvas import _element_screenshot
from browser_protocol import BrowserRuntimeError, WebDriverClient
from browser_trace import Trace


CONTEXT_NAME_PATTERN = re.compile(r"[a-z][a-z0-9-]{0,31}\Z")


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
    _element_screenshot(client, trace, directory, canvas_id, client.find("#" + canvas_id))
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
            client, trace, directory, f"{canvas_id}-after-rejections", expected, context_name
        )
        if actual["screenshot_sha256"] != expected["screenshot_sha256"]:
            raise BrowserRuntimeError(
                f"a rejected file batch changed canvas {canvas_id} screenshot"
            )
