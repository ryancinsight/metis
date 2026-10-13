"""Compute the input evidence schema from observed native workflow values."""
from __future__ import annotations

import hashlib
from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class InputObservation:
    encoded: bytes
    units: tuple[int, ...]
    text_supplied: bool
    shortcut: str | None
    layout: int
    focus: dict[str, Any]
    foreground_set: bool
    scroll: dict[str, Any] | None
    click: dict[str, Any] | None


def scroll(point: tuple[int, int], notches: int, settle_rounds: int,
           frame_changed: bool) -> dict[str, Any]:
    return {
        "mechanism": "Win32 SendInput MOUSEEVENTF_WHEEL",
        "client_point": list(point),
        "notches": notches,
        "settle_rounds": settle_rounds,
        "frame_changed": frame_changed,
    }


def click(point: tuple[int, int], screen_point: tuple[int, int]) -> dict[str, Any]:
    return {
        "mechanism": "Win32 SendInput MOUSEEVENTF_ABSOLUTE",
        "client_point": list(point),
        "screen_point": list(screen_point),
    }


def record(observation: InputObservation) -> dict[str, Any]:
    mechanisms = []
    if observation.text_supplied:
        mechanisms.append("Win32 SendInput KEYEVENTF_UNICODE")
    if observation.shortcut is not None:
        mechanisms.append("Win32 SendInput virtual key")
    if observation.click is not None:
        mechanisms.append("Win32 SendInput MOUSEEVENTF_ABSOLUTE")
    return {
        "input": {
            "mechanism": " + ".join(mechanisms),
            "utf8_bytes": len(observation.encoded),
            "utf16_units": len(observation.units),
            "text_sha256": (
                hashlib.sha256(observation.encoded).hexdigest()
                if observation.text_supplied else None
            ),
            "target_keyboard_layout": f"0x{observation.layout:016x}",
            "keyboard_focus": observation.focus,
            "foreground_window_set": observation.foreground_set,
            "shortcut": observation.shortcut,
            "scroll": observation.scroll,
            "click": observation.click,
        },
        "ime": {
            "status": "not_exercised",
            "reason": (
                "KEYEVENTF_UNICODE delivers committed text; an installed IME "
                "requires a separate host journey"
            ),
        },
    }
