"""Capture a visible Rust-owned native frame from an extracted Metis wheel."""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import importlib
import json
import os
import pathlib
import struct
import subprocess
import sys
import tempfile
import zipfile
from concurrent.futures import Future, ThreadPoolExecutor
from dataclasses import dataclass
from typing import Any

from python_binding import extract_wheel, validate_wheel_surface


MAX_PUMP_ROUNDS = 300
EVENT_WAIT_MILLISECONDS = 100
PRINT_WINDOW_FULL_CONTENT = 2


class _BitmapInfoHeader(ctypes.Structure):
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


class _BitmapInfo(ctypes.Structure):
    _fields_ = [("header", _BitmapInfoHeader), ("colors", ctypes.c_uint32 * 3)]


@dataclass(frozen=True)
class _WindowBounds:
    handle: int
    width: int
    height: int


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Capture a visible NativeApplication frame as a Windows BMP."
    )
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--wheel", type=pathlib.Path)
    source.add_argument("--site", type=pathlib.Path, help=argparse.SUPPRESS)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--title", default="Metis Python native capture")
    parser.add_argument("--width", type=int, default=320)
    parser.add_argument("--height", type=int, default=240)
    return parser


def _load_site(site: pathlib.Path) -> Any:
    sys.path.insert(0, str(site))
    return importlib.import_module("metis")


def _window_for_process(process_id: int) -> _WindowBounds:
    if sys.platform != "win32":
        raise RuntimeError("visible native capture requires Windows")

    user32 = ctypes.windll.user32
    callback_type = ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)
    user32.EnumWindows.argtypes = [callback_type, ctypes.c_void_p]
    user32.GetWindowThreadProcessId.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(ctypes.c_uint32),
    ]
    user32.GetWindowThreadProcessId.restype = ctypes.c_uint32
    user32.IsWindowVisible.argtypes = [ctypes.c_void_p]
    user32.IsWindowVisible.restype = ctypes.c_bool
    user32.GetWindowRect.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(ctypes.c_long * 4),
    ]
    user32.GetWindowRect.restype = ctypes.c_bool
    found: list[_WindowBounds] = []

    @callback_type
    def visit(hwnd: int, _lparam: int) -> bool:
        owner = ctypes.c_uint32()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value != process_id or not user32.IsWindowVisible(hwnd):
            return True
        rectangle = (ctypes.c_long * 4)()
        if not user32.GetWindowRect(hwnd, ctypes.byref(rectangle)):
            return True
        width = rectangle[2] - rectangle[0]
        height = rectangle[3] - rectangle[1]
        if width > 0 and height > 0:
            found.append(_WindowBounds(int(hwnd), width, height))
            return False
        return True

    user32.EnumWindows(visit, None)
    if not found:
        raise RuntimeError("visible NativeApplication window was not discoverable")
    return found[0]


def _capture_window(bounds: _WindowBounds) -> bytes:
    """Render one HWND through GDI; the caller pumps its owner thread."""
    if sys.platform != "win32":
        raise RuntimeError("visible native capture requires Windows")

    user32 = ctypes.windll.user32
    gdi32 = ctypes.windll.gdi32
    user32.GetWindowDC.argtypes = [ctypes.c_void_p]
    user32.GetWindowDC.restype = ctypes.c_void_p
    user32.ReleaseDC.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    user32.ReleaseDC.restype = ctypes.c_int
    user32.PrintWindow.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint32]
    user32.PrintWindow.restype = ctypes.c_bool
    gdi32.CreateCompatibleDC.argtypes = [ctypes.c_void_p]
    gdi32.CreateCompatibleDC.restype = ctypes.c_void_p
    gdi32.CreateCompatibleBitmap.argtypes = [
        ctypes.c_void_p,
        ctypes.c_int,
        ctypes.c_int,
    ]
    gdi32.CreateCompatibleBitmap.restype = ctypes.c_void_p
    gdi32.SelectObject.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    gdi32.SelectObject.restype = ctypes.c_void_p
    gdi32.DeleteObject.argtypes = [ctypes.c_void_p]
    gdi32.DeleteObject.restype = ctypes.c_bool
    gdi32.DeleteDC.argtypes = [ctypes.c_void_p]
    gdi32.DeleteDC.restype = ctypes.c_bool
    gdi32.GetDIBits.argtypes = [
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_void_p,
        ctypes.POINTER(_BitmapInfo),
        ctypes.c_uint32,
    ]
    gdi32.GetDIBits.restype = ctypes.c_int

    window_dc = user32.GetWindowDC(bounds.handle)
    if not window_dc:
        raise ctypes.WinError()
    memory_dc = gdi32.CreateCompatibleDC(window_dc)
    bitmap = gdi32.CreateCompatibleBitmap(window_dc, bounds.width, bounds.height)
    if not memory_dc or not bitmap:
        if bitmap:
            gdi32.DeleteObject(bitmap)
        if memory_dc:
            gdi32.DeleteDC(memory_dc)
        user32.ReleaseDC(bounds.handle, window_dc)
        raise ctypes.WinError()
    previous = gdi32.SelectObject(memory_dc, bitmap)
    try:
        if not user32.PrintWindow(
            bounds.handle, memory_dc, PRINT_WINDOW_FULL_CONTENT
        ):
            raise ctypes.WinError()
        info = _BitmapInfo()
        info.header = _BitmapInfoHeader(
            ctypes.sizeof(_BitmapInfoHeader),
            bounds.width,
            -bounds.height,
            1,
            32,
            0,
            0,
            0,
            0,
            0,
            0,
        )
        pixels = ctypes.create_string_buffer(bounds.width * bounds.height * 4)
        lines = gdi32.GetDIBits(
            memory_dc,
            bitmap,
            0,
            bounds.height,
            pixels,
            ctypes.byref(info),
            0,
        )
        if lines != bounds.height:
            raise ctypes.WinError()
        return bytes(pixels)
    finally:
        gdi32.SelectObject(memory_dc, previous)
        gdi32.DeleteObject(bitmap)
        gdi32.DeleteDC(memory_dc)
        user32.ReleaseDC(bounds.handle, window_dc)


def _write_bmp(path: pathlib.Path, bounds: _WindowBounds, pixels: bytes) -> str:
    image_size = len(pixels)
    file_size = 14 + ctypes.sizeof(_BitmapInfoHeader) + image_size
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


def _checkerboard(width: int, height: int) -> bytes:
    red = bytes((229, 62, 62, 255)) * (width // 2)
    blue = bytes((49, 130, 206, 255)) * (width - width // 2)
    return b"".join((red + blue) if row % 2 == 0 else (blue + red) for row in range(height))


def _capture(metis: Any, title: str, width: int, height: int, output: pathlib.Path) -> dict[str, Any]:
    host = metis.NativeApplication(title, width, height, "visible")
    generation = host.generation
    executor = ThreadPoolExecutor(max_workers=1)
    future: Future[bytes] | None = None
    try:
        host.present(generation, _checkerboard(width, height))
        events = host.wait_events(generation, 0)
        bounds = _window_for_process(os.getpid())
        future = executor.submit(_capture_window, bounds)
        for _ in range(MAX_PUMP_ROUNDS):
            if future.done():
                break
            host.wait_events(generation, EVENT_WAIT_MILLISECONDS)
        if future is None or not future.done():
            raise RuntimeError("native capture did not complete within the pump bound")
        pixels = future.result()
        digest = _write_bmp(output, bounds, pixels)
        return {
            "title": title,
            "generation": generation,
            "window": {"width": bounds.width, "height": bounds.height},
            "events": events,
            "image": output.as_posix(),
            "sha256": digest,
        }
    finally:
        if future is not None and not future.done():
            future.cancel()
        executor.shutdown(wait=True, cancel_futures=True)
        host.close(generation)


def main() -> None:
    """Build no code; capture one visible frame from a supplied wheel."""
    if sys.platform != "win32":
        raise SystemExit("python_native_capture.py requires a Windows desktop")
    arguments = _parser().parse_args()
    if arguments.width <= 0 or arguments.height <= 0:
        raise SystemExit("width and height must be positive")
    output = arguments.output.resolve()
    if arguments.site is not None:
        if not arguments.site.is_dir():
            raise SystemExit(f"Python wheel site does not exist: {arguments.site}")
        metis = _load_site(arguments.site.resolve())
        result = _capture(metis, arguments.title, arguments.width, arguments.height, output)
        print(json.dumps(result, sort_keys=True))
        return

    wheel = arguments.wheel.resolve()
    if not wheel.is_file() or wheel.suffix != ".whl":
        raise SystemExit(f"wheel does not exist: {wheel}")
    with tempfile.TemporaryDirectory(prefix="metis-python-native-") as temporary:
        site = pathlib.Path(temporary)
        validate_wheel_surface(wheel)
        extract_wheel(wheel, site)
        command = [
            sys.executable,
            str(pathlib.Path(__file__).resolve()),
            "--site",
            str(site),
            "--output",
            str(output),
            "--title",
            arguments.title,
            "--width",
            str(arguments.width),
            "--height",
            str(arguments.height),
        ]
        completed = subprocess.run(command, check=False, text=True, capture_output=True)
        if completed.returncode != 0:
            if completed.stdout:
                print(completed.stdout, end="")
            if completed.stderr:
                print(completed.stderr, file=sys.stderr, end="")
            raise SystemExit(completed.returncode)
        print(completed.stdout, end="")


if __name__ == "__main__":
    main()
