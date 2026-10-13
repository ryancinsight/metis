"""Canonical captured-image file dispatch by admitted output extension."""
from __future__ import annotations
import pathlib
from native_capture import model
from native_capture.images import bmp, png


def write_capture(path: pathlib.Path, bounds: model.WindowBounds, pixels: bytes) -> str:
    """Write a capture in the format selected by its file extension."""
    suffix = path.suffix.lower()
    if suffix == ".bmp":
        return bmp.write_bmp(path, bounds, pixels)
    if suffix == ".png":
        return png.write_png(path, bounds, pixels)
    raise ValueError("capture output must use the .bmp or .png extension")
