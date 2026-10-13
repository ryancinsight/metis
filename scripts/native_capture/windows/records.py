"""Windows bitmap and process-snapshot ABI records; no DLL access at import."""
from __future__ import annotations
import ctypes


class BitmapInfoHeader(ctypes.Structure):
    _fields_ = [
        ("size", ctypes.c_uint32),
        ("width", ctypes.c_int32),
        ("height", ctypes.c_int32),
        ("planes", ctypes.c_uint16),
        ("bits_per_pixel", ctypes.c_uint16),
        ("compression", ctypes.c_uint32),
        ("image_size", ctypes.c_uint32),
        ("x_pixels_per_meter", ctypes.c_int32),
        ("y_pixels_per_meter", ctypes.c_int32),
        ("colors_used", ctypes.c_uint32),
        ("important_colors", ctypes.c_uint32),
    ]


class BitmapInfo(ctypes.Structure):
    _fields_ = [("header", BitmapInfoHeader), ("colors", ctypes.c_uint32 * 3)]


class ProcessEntry32W(ctypes.Structure):
    _fields_ = [
        ("size", ctypes.c_uint32),
        ("usage", ctypes.c_uint32),
        ("process_id", ctypes.c_uint32),
        ("default_heap_id", ctypes.c_size_t),
        ("module_id", ctypes.c_uint32),
        ("threads", ctypes.c_uint32),
        ("parent_process_id", ctypes.c_uint32),
        ("priority", ctypes.c_int32),
        ("flags", ctypes.c_uint32),
        ("executable", ctypes.c_wchar * 260),
    ]
