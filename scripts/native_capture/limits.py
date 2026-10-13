"""Authoritative capture dimensions, resource bounds and lifecycle budgets."""
from __future__ import annotations


MAX_PUMP_ROUNDS = 300


EVENT_WAIT_MILLISECONDS = 100


PROCESS_WINDOW_TIMEOUT_SECONDS = 30


PROCESS_EXIT_TIMEOUT_SECONDS = 10


MAX_INPUT_BYTES = 64 * 1024 * 1024


MAX_FRAME_PIXELS = 16 * 1024 * 1024


MAX_FRAME_DIMENSION = 16_384


def validate_dimensions(width: int, height: int, label: str) -> None:
    if width <= 0 or height <= 0:
        raise ValueError(f"{label} dimensions must be positive")
    if width > MAX_FRAME_DIMENSION or height > MAX_FRAME_DIMENSION:
        raise ValueError(f"{label} dimensions exceed the bounded capture limit")
    if width * height > MAX_FRAME_PIXELS:
        raise ValueError(f"{label} pixel count exceeds the bounded capture limit")
