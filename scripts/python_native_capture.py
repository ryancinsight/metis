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
import time
import zipfile
import zlib
from concurrent.futures import Future, ThreadPoolExecutor
from dataclasses import dataclass
from typing import Any

from python_binding import extract_wheel, validate_wheel_surface


MAX_PUMP_ROUNDS = 300
EVENT_WAIT_MILLISECONDS = 100
# The Moirai presenter handles the standard WM_PRINT path. PW_RENDERFULLCONTENT
# is intended for framework-managed surfaces and bypasses this custom GDI client
# render on the Windows host, yielding a black client area.
PRINT_WINDOW_FLAGS = 0
PROCESS_WINDOW_TIMEOUT_SECONDS = 30
PROCESS_EXIT_TIMEOUT_SECONDS = 10
DEFAULT_WIDTH = 320
DEFAULT_HEIGHT = 240
MAX_INPUT_BYTES = 64 * 1024 * 1024
MAX_FRAME_PIXELS = 16 * 1024 * 1024
MAX_FRAME_DIMENSION = 16_384
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


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


class _ProcessEntry32W(ctypes.Structure):
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


@dataclass(frozen=True)
class _WindowBounds:
    handle: int
    width: int
    height: int


@dataclass(frozen=True)
class _Frame:
    """Bounded row-major RGBA pixels ready for the Rust host."""

    width: int
    height: int
    rgba: bytes
    sha256: str


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Capture a visible NativeApplication frame as a Windows BMP or PNG."
    )
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--wheel", type=pathlib.Path)
    source.add_argument("--site", type=pathlib.Path, help=argparse.SUPPRESS)
    source.add_argument(
        "--command",
        type=pathlib.Path,
        help="launch one visible native process and capture its first window",
    )
    parser.add_argument(
        "--argument",
        action="append",
        default=[],
        dest="command_arguments",
        help="one argument passed to --command; repeat for each argument",
    )
    parser.add_argument(
        "--cwd",
        type=pathlib.Path,
        help="working directory for --command",
    )
    parser.add_argument(
        "--frame",
        type=pathlib.Path,
        help="an existing RGBA PNG frame to present through NativeApplication",
    )
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--title", default="Metis Python native capture")
    parser.add_argument("--width", type=int)
    parser.add_argument("--height", type=int)
    return parser


def _load_site(site: pathlib.Path) -> Any:
    sys.path.insert(0, str(site))
    return importlib.import_module("metis")


def _window_for_processes(process_ids: set[int]) -> _WindowBounds:
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
        if owner.value not in process_ids or not user32.IsWindowVisible(hwnd):
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
        raise RuntimeError("visible process window was not discoverable")
    return found[0]


def _window_for_process(process_id: int) -> _WindowBounds:
    """Find a visible window owned by one exact process."""
    return _window_for_processes({process_id})


def _process_tree(root_process_id: int) -> set[int]:
    """Return the root process and all currently live descendants."""
    kernel32 = ctypes.windll.kernel32
    kernel32.CreateToolhelp32Snapshot.argtypes = [ctypes.c_uint32, ctypes.c_uint32]
    kernel32.CreateToolhelp32Snapshot.restype = ctypes.c_void_p
    snapshot = kernel32.CreateToolhelp32Snapshot(0x00000002, 0)
    invalid_handle = ctypes.c_void_p(-1).value
    if snapshot == invalid_handle:
        raise ctypes.WinError()
    kernel32.Process32FirstW.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(_ProcessEntry32W),
    ]
    kernel32.Process32FirstW.restype = ctypes.c_bool
    kernel32.Process32NextW.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(_ProcessEntry32W),
    ]
    kernel32.Process32NextW.restype = ctypes.c_bool
    kernel32.CloseHandle.argtypes = [ctypes.c_void_p]
    kernel32.CloseHandle.restype = ctypes.c_bool
    children: dict[int, list[int]] = {}
    entry = _ProcessEntry32W(size=ctypes.sizeof(_ProcessEntry32W))
    try:
        first = kernel32.Process32FirstW(snapshot, ctypes.byref(entry))
        if not first:
            raise ctypes.WinError()
        while True:
            children.setdefault(entry.parent_process_id, []).append(entry.process_id)
            entry.size = ctypes.sizeof(_ProcessEntry32W)
            if not kernel32.Process32NextW(snapshot, ctypes.byref(entry)):
                break
    finally:
        kernel32.CloseHandle(snapshot)
    process_ids = {root_process_id}
    pending = [root_process_id]
    while pending:
        parent = pending.pop()
        for child in children.get(parent, []):
            if child not in process_ids:
                process_ids.add(child)
                pending.append(child)
    return process_ids


def _wait_for_process_window(process: subprocess.Popen[bytes]) -> _WindowBounds:
    """Wait for one visible process window using bounded Win32 readiness."""
    if sys.platform != "win32":
        raise RuntimeError("visible native capture requires Windows")

    user32 = ctypes.windll.user32
    kernel32 = ctypes.windll.kernel32
    process_query_limited_information = 0x1000
    synchronize = 0x00100000
    user32.WaitForInputIdle.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
    user32.WaitForInputIdle.restype = ctypes.c_uint32
    kernel32.OpenProcess.argtypes = [ctypes.c_uint32, ctypes.c_bool, ctypes.c_uint32]
    kernel32.OpenProcess.restype = ctypes.c_void_p
    kernel32.CloseHandle.argtypes = [ctypes.c_void_p]
    kernel32.CloseHandle.restype = ctypes.c_bool

    deadline = time.monotonic() + PROCESS_WINDOW_TIMEOUT_SECONDS
    handle = kernel32.OpenProcess(
        process_query_limited_information | synchronize,
        False,
        process.pid,
    )
    if not handle:
        raise ctypes.WinError()
    try:
        remaining = max(0.0, deadline - time.monotonic())
        result = user32.WaitForInputIdle(handle, int(remaining * 1000))
    finally:
        kernel32.CloseHandle(handle)
    if result == 0x00000102:
        raise TimeoutError("native process did not become input-idle before the capture deadline")
    if result == 0xFFFFFFFF and process.poll() is not None:
        raise RuntimeError("native process exited before creating a visible window")

    while time.monotonic() < deadline:
        try:
            return _window_for_processes(_process_tree(process.pid))
        except RuntimeError:
            if process.poll() is not None:
                raise RuntimeError("native process exited before creating a visible window")
            time.sleep(EVENT_WAIT_MILLISECONDS / 1000)
    raise TimeoutError("native process window was not discoverable before the capture deadline")


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
        if not user32.PrintWindow(bounds.handle, memory_dc, PRINT_WINDOW_FLAGS):
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


def _close_window(handle: int) -> None:
    """Request orderly close of a captured native window."""
    user32 = ctypes.windll.user32
    user32.PostMessageW.argtypes = [
        ctypes.c_void_p,
        ctypes.c_uint32,
        ctypes.c_size_t,
        ctypes.c_ssize_t,
    ]
    user32.PostMessageW.restype = ctypes.c_bool
    if not user32.PostMessageW(handle, 0x0010, 0, 0):
        raise ctypes.WinError()


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


def _write_png(path: pathlib.Path, bounds: _WindowBounds, pixels: bytes) -> str:
    """Write the captured top-down BGRA rows as an RGBA PNG."""
    expected = bounds.width * bounds.height * 4
    if len(pixels) != expected:
        raise ValueError("captured pixel storage does not match the window bounds")
    rows = bytearray()
    row_bytes = bounds.width * 4
    for offset in range(0, len(pixels), row_bytes):
        source = pixels[offset : offset + row_bytes]
        rows.append(0)
        for blue, green, red, _alpha in zip(
            source[0::4], source[1::4], source[2::4], source[3::4], strict=True
        ):
            rows.extend((red, green, blue, 255))
    signature = b"\x89PNG\r\n\x1a\n"
    header = struct.pack(">IIBBBBB", bounds.width, bounds.height, 8, 6, 0, 0, 0)

    def chunk(kind: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
        )

    content = signature + chunk(b"IHDR", header)
    content += chunk(b"IDAT", zlib.compress(bytes(rows)))
    content += chunk(b"IEND", b"")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    return hashlib.sha256(content).hexdigest()


def _write_capture(path: pathlib.Path, bounds: _WindowBounds, pixels: bytes) -> str:
    """Write a capture in the format selected by its file extension."""
    suffix = path.suffix.lower()
    if suffix == ".bmp":
        return _write_bmp(path, bounds, pixels)
    if suffix == ".png":
        return _write_png(path, bounds, pixels)
    raise ValueError("capture output must use the .bmp or .png extension")


def _paeth(left: int, above: int, upper_left: int) -> int:
    estimate = left + above - upper_left
    left_distance = abs(estimate - left)
    above_distance = abs(estimate - above)
    upper_left_distance = abs(estimate - upper_left)
    if left_distance <= above_distance and left_distance <= upper_left_distance:
        return left
    if above_distance <= upper_left_distance:
        return above
    return upper_left


def _read_png(path: pathlib.Path) -> _Frame:
    """Read a bounded, non-interlaced 8-bit RGBA PNG into row-major bytes.

    This is the interchange boundary for an already-decoded application frame;
    it never inspects DICOM or other medical-format data. The decoder accepts
    all PNG scanline filters so a RITK framebuffer capture can be handed to the
    Python host without a third-party image package.
    """
    path = path.resolve(strict=True)
    if not path.is_file():
        raise ValueError(f"frame is not a regular file: {path}")
    size = path.stat().st_size
    if size > MAX_INPUT_BYTES:
        raise ValueError("frame PNG exceeds the bounded input size")
    content = path.read_bytes()
    if len(content) != size or len(content) < len(PNG_SIGNATURE) or content[:8] != PNG_SIGNATURE:
        raise ValueError("frame is not a complete PNG")

    offset = len(PNG_SIGNATURE)
    width = height = None
    compressed = bytearray()
    saw_idat = False
    saw_iend = False
    while offset < len(content):
        if len(content) - offset < 12:
            raise ValueError("frame PNG has a truncated chunk")
        length = struct.unpack_from(">I", content, offset)[0]
        end = offset + 12 + length
        if end > len(content):
            raise ValueError("frame PNG has a truncated payload")
        kind = content[offset + 4 : offset + 8]
        payload_start = offset + 8
        payload = content[payload_start : payload_start + length]
        checksum = struct.unpack_from(">I", content, payload_start + length)[0]
        if zlib.crc32(kind + payload) & 0xFFFFFFFF != checksum:
            raise ValueError("frame PNG has an invalid chunk checksum")
        if kind == b"IHDR":
            if offset != len(PNG_SIGNATURE) or width is not None or length != 13:
                raise ValueError("frame PNG has an invalid image header")
            width, height, depth, color_type, compression, filtering, interlace = struct.unpack(
                ">IIBBBBB", payload
            )
            if (
                width == 0
                or height == 0
                or width > MAX_FRAME_DIMENSION
                or height > MAX_FRAME_DIMENSION
                or width * height > MAX_FRAME_PIXELS
                or (depth, color_type, compression, filtering, interlace) != (8, 6, 0, 0, 0)
            ):
                raise ValueError("frame PNG must be a bounded non-interlaced RGBA image")
        elif kind == b"IDAT":
            if width is None:
                raise ValueError("frame PNG data appears before its image header")
            saw_idat = True
            compressed.extend(payload)
        elif kind == b"IEND":
            if length != 0 or not saw_idat:
                raise ValueError("frame PNG is missing image data")
            saw_iend = True
            offset = end
            break
        offset = end

    if width is None or height is None or not saw_iend or offset != len(content):
        raise ValueError("frame PNG is missing its terminal chunk")
    row_bytes = width * 4
    raw_size = height * (row_bytes + 1)
    decoder = zlib.decompressobj()
    raw = decoder.decompress(bytes(compressed), raw_size + 1)
    if len(raw) > raw_size or decoder.unconsumed_tail or not decoder.eof:
        raise ValueError("frame PNG decompression exceeds the bounded image size")
    if decoder.unused_data:
        raise ValueError("frame PNG contains trailing compressed data")
    raw += decoder.flush(raw_size + 1 - len(raw))
    if len(raw) != raw_size:
        raise ValueError("frame PNG scanlines do not match the image dimensions")

    pixels = bytearray(width * height * 4)
    previous = bytearray(row_bytes)
    raw_offset = 0
    output_offset = 0
    for _row in range(height):
        filter_type = raw[raw_offset]
        scanline = raw[raw_offset + 1 : raw_offset + 1 + row_bytes]
        reconstructed = bytearray(row_bytes)
        if filter_type == 0:
            reconstructed[:] = scanline
        elif filter_type == 1:
            for index, value in enumerate(scanline):
                left = reconstructed[index - 4] if index >= 4 else 0
                reconstructed[index] = (value + left) & 0xFF
        elif filter_type == 2:
            for index, value in enumerate(scanline):
                reconstructed[index] = (value + previous[index]) & 0xFF
        elif filter_type == 3:
            for index, value in enumerate(scanline):
                left = reconstructed[index - 4] if index >= 4 else 0
                reconstructed[index] = (value + ((left + previous[index]) // 2)) & 0xFF
        elif filter_type == 4:
            for index, value in enumerate(scanline):
                left = reconstructed[index - 4] if index >= 4 else 0
                above = previous[index]
                upper_left = previous[index - 4] if index >= 4 else 0
                reconstructed[index] = (value + _paeth(left, above, upper_left)) & 0xFF
        else:
            raise ValueError("frame PNG uses an unsupported scanline filter")
        pixels[output_offset : output_offset + row_bytes] = reconstructed
        previous = reconstructed
        raw_offset += row_bytes + 1
        output_offset += row_bytes
    return _Frame(width, height, bytes(pixels), hashlib.sha256(content).hexdigest())


def _checkerboard(width: int, height: int) -> bytes:
    red = bytes((229, 62, 62, 255)) * (width // 2)
    blue = bytes((49, 130, 206, 255)) * (width - width // 2)
    return b"".join((red + blue) if row % 2 == 0 else (blue + red) for row in range(height))


def _capture(
    metis: Any,
    title: str,
    width: int,
    height: int,
    output: pathlib.Path,
    frame: _Frame | None = None,
) -> dict[str, Any]:
    host = metis.NativeApplication(title, width, height, "visible")
    generation = host.generation
    executor = ThreadPoolExecutor(max_workers=1)
    future: Future[bytes] | None = None
    try:
        presented = _checkerboard(width, height) if frame is None else frame.rgba
        host.present(generation, presented)
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
        digest = _write_capture(output, bounds, pixels)
        result = {
            "title": title,
            "generation": generation,
            "window": {"width": bounds.width, "height": bounds.height},
            "events": events,
            "image": output.as_posix(),
            "sha256": digest,
        }
        if frame is None:
            result["input"] = {
                "format": "rgba",
                "width": width,
                "height": height,
                "source": "checkerboard",
            }
        else:
            result["input"] = {
                "format": "png-rgba",
                "width": frame.width,
                "height": frame.height,
                "sha256": frame.sha256,
            }
        return result
    finally:
        if future is not None and not future.done():
            future.cancel()
        executor.shutdown(wait=True, cancel_futures=True)
        host.close(generation)


def _capture_command(
    command: pathlib.Path,
    command_arguments: list[str],
    cwd: pathlib.Path | None,
    output: pathlib.Path,
) -> dict[str, Any]:
    """Launch one visible process, capture its full window and close it."""
    if sys.platform != "win32":
        raise RuntimeError("visible native capture requires Windows")
    executable = command.resolve(strict=True)
    if cwd is not None:
        cwd = cwd.resolve(strict=True)
        if not cwd.is_dir():
            raise NotADirectoryError(cwd)
    process = subprocess.Popen(
        [str(executable), *command_arguments],
        cwd=cwd,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    bounds: _WindowBounds | None = None
    try:
        bounds = _wait_for_process_window(process)
        pixels = _capture_window(bounds)
        digest = _write_capture(output, bounds, pixels)
        _close_window(bounds.handle)
        return_code = process.wait(timeout=PROCESS_EXIT_TIMEOUT_SECONDS)
        if return_code != 0:
            raise RuntimeError(f"native process exited with status {return_code}")
        return {
            "process_returncode": return_code,
            "window": {"width": bounds.width, "height": bounds.height},
            "image": output.as_posix(),
            "sha256": digest,
        }
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=PROCESS_EXIT_TIMEOUT_SECONDS)


def main() -> None:
    """Capture one visible frame from a supplied wheel or native command."""
    if sys.platform != "win32":
        raise SystemExit("python_native_capture.py requires a Windows desktop")
    arguments = _parser().parse_args()
    if (arguments.width is None) != (arguments.height is None):
        raise SystemExit("--width and --height must be supplied together")
    if arguments.width is not None and (arguments.width <= 0 or arguments.height <= 0):
        raise SystemExit("width and height must be positive")
    if arguments.command is None and (
        arguments.command_arguments or arguments.cwd is not None
    ):
        raise SystemExit("--argument and --cwd require --command")
    if arguments.frame is not None and arguments.command is not None:
        raise SystemExit("--frame cannot be combined with --command")
    frame = None if arguments.frame is None else _read_png(arguments.frame)
    if frame is None:
        width = DEFAULT_WIDTH if arguments.width is None else arguments.width
        height = DEFAULT_HEIGHT if arguments.height is None else arguments.height
    else:
        width, height = frame.width, frame.height
        if arguments.width is not None and (arguments.width, arguments.height) != (width, height):
            raise SystemExit("--width and --height must match the supplied frame")
    output = arguments.output.resolve()
    if arguments.site is not None:
        if not arguments.site.is_dir():
            raise SystemExit(f"Python wheel site does not exist: {arguments.site}")
        metis = _load_site(arguments.site.resolve())
        result = _capture(metis, arguments.title, width, height, output, frame)
        print(json.dumps(result, sort_keys=True))
        return

    if arguments.command is not None:
        if arguments.wheel is not None or arguments.site is not None:
            raise SystemExit("--command cannot be combined with a wheel or site")
        result = _capture_command(
            arguments.command,
            arguments.command_arguments,
            arguments.cwd,
            output,
        )
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
            str(width),
            "--height",
            str(height),
        ]
        if arguments.frame is not None:
            command.extend(("--frame", str(arguments.frame.resolve(strict=True))))
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
