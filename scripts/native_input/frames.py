"""Bounded frame readiness and revision-bound capture records."""
from __future__ import annotations
import pathlib
from typing import Any


SETTLE_ROUNDS = 100


SETTLE_WAIT_SECONDS = 0.1


STABLE_ROUNDS = 5


SETTLE_HISTORY = 4


POST_INPUT_MIN_OBSERVE_SECONDS = 5.0


def after_settled(
    *,
    changed: bool,
    repeated: bool,
    observed_seconds: float,
    minimum_seconds: float,
) -> bool:
    """Whether the post-input observation may end.

    The after frame must show the input's response, which may render after
    the input queue drains, so a frame that merely holds still is not
    evidence: the observation ends when a frame that changed from the state
    the input left behind also repeats among the recent frames (the
    response arrived and now holds or cycles, as a blinking caret does),
    or, once the minimum observation window has passed, when the surface
    repeats with nothing new arriving.
    """
    if not repeated:
        return False
    return changed or observed_seconds >= minimum_seconds


def blank_frame(pixels: bytes, width: int, height: int) -> bool:
    """A frame whose center half is uniform carries no rendered client yet.

    The outer window frame includes title bar, borders and desktop shadow,
    so readiness reads the central region that lies inside the client area
    of every recorded host instead of the chrome around it.
    """
    if width < 4 or height < 4 or len(pixels) < width * height * 4:
        return True
    stride = width * 4
    x0, x1 = width // 4, width - width // 4
    y0, y1 = height // 4, height - height // 4
    reference = pixels[y0 * stride + x0 * 4 : y0 * stride + x0 * 4 + 4]
    for y in range(y0, y1):
        row = pixels[y * stride : (y + 1) * stride]
        for x in range(x0, x1):
            if row[x * 4 : x * 4 + 4] != reference:
                return False
    return True


def record(
    observation: Any,
    image: pathlib.Path,
    digest: str,
) -> dict[str, Any]:
    return {
        "window": {
            "outer": [observation.bounds.width, observation.bounds.height],
            "client": [observation.client_width, observation.client_height],
            "dpi": observation.dpi,
        },
        "image": image.as_posix(),
        "sha256": digest,
    }
