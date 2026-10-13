"""Encode a captured top-down BGRA buffer as a Windows BMP."""
from __future__ import annotations
import ctypes
import hashlib
import pathlib
import struct
from native_capture import model
from native_capture.windows import records


def write_bmp(path: pathlib.Path, bounds: model.WindowBounds, pixels: bytes) -> str:
    image_size = len(pixels)
    file_size = 14 + ctypes.sizeof(records.BitmapInfoHeader) + image_size
    file_header = struct.pack("<HIHHI", 0x4D42, file_size, 0, 0, 54)
    info_header = struct.pack(
        "<IiiHHIIiiII",
        40,
        bounds.width,
        -bounds.height,
        1,
        32,
        0,
        image_size,
        0,
        0,
        0,
        0,
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(file_header + info_header + pixels)
    return hashlib.sha256(path.read_bytes()).hexdigest()
