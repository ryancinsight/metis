"""Immutable capture geometry and bounded decoded frame values."""
from __future__ import annotations
from dataclasses import dataclass


@dataclass(frozen=True)
class WindowBounds:
    handle: int
    width: int
    height: int


@dataclass(frozen=True)
class WindowObservation:
    bounds: WindowBounds
    client_width: int
    client_height: int
    dpi: int


@dataclass(frozen=True)
class Frame:
    """Bounded row-major RGBA pixels ready for the Rust host."""

    width: int
    height: int
    rgba: bytes
    sha256: str
