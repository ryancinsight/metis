"""Physical Win32 PrintWindow and visible-screen pixel acquisition."""
from __future__ import annotations
import ctypes
import sys
from native_capture import model
from native_input.windows import abi
from native_capture.windows import records
from native_display import physical_coordinates


# The Moirai presenter handles standard WM_PRINT. PW_RENDERFULLCONTENT
# bypasses this custom GDI client render and produces a black client area.
PRINT_WINDOW_FLAGS = 0


@physical_coordinates()
def capture_window(bounds: model.WindowBounds) -> bytes:
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
    user32.PrintWindow.restype = ctypes.c_int32
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
    gdi32.DeleteObject.restype = ctypes.c_int32
    gdi32.DeleteDC.argtypes = [ctypes.c_void_p]
    gdi32.DeleteDC.restype = ctypes.c_int32
    gdi32.GetDIBits.argtypes = [
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_void_p,
        ctypes.POINTER(records.BitmapInfo),
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
        info = records.BitmapInfo()
        info.header = records.BitmapInfoHeader(
            ctypes.sizeof(records.BitmapInfoHeader),
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


def capture_screen_window(bounds: model.WindowBounds) -> bytes:
    """Capture the visible screen pixels under a window rect through GDI.

    The window must be shown, focused and unobstructed; the caller owns focus
    and stacking. Composition surfaces such as the WebView2 host render no
    GDI content into a `WM_PRINT` context, so their captures read the pixels
    the desktop actually presents. The returned bytes keep the same top-down
    BGRA row format as [`capture_window`].
    """
    if sys.platform != "win32":
        raise RuntimeError("visible native capture requires Windows")

    user32 = ctypes.windll.user32
    gdi32 = ctypes.windll.gdi32
    user32.GetWindowRect.argtypes = [ctypes.c_void_p, ctypes.POINTER(abi.Rect)]
    user32.GetWindowRect.restype = ctypes.c_int32
    user32.GetDC.argtypes = [ctypes.c_void_p]
    user32.GetDC.restype = ctypes.c_void_p
    user32.ReleaseDC.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    user32.ReleaseDC.restype = ctypes.c_int
    gdi32.CreateCompatibleDC.argtypes = [ctypes.c_void_p]
    gdi32.CreateCompatibleDC.restype = ctypes.c_void_p
    gdi32.CreateCompatibleBitmap.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int]
    gdi32.CreateCompatibleBitmap.restype = ctypes.c_void_p
    gdi32.SelectObject.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    gdi32.SelectObject.restype = ctypes.c_void_p
    gdi32.BitBlt.argtypes = [
        ctypes.c_void_p,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_void_p,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_uint32,
    ]
    gdi32.BitBlt.restype = ctypes.c_int32
    gdi32.DeleteObject.argtypes = [ctypes.c_void_p]
    gdi32.DeleteObject.restype = ctypes.c_int32
    gdi32.DeleteDC.argtypes = [ctypes.c_void_p]
    gdi32.DeleteDC.restype = ctypes.c_int32
    gdi32.GetDIBits.argtypes = [
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_void_p,
        ctypes.POINTER(records.BitmapInfo),
        ctypes.c_uint32,
    ]
    gdi32.GetDIBits.restype = ctypes.c_int

    rect = abi.Rect()
    if not user32.GetWindowRect(bounds.handle, ctypes.byref(rect)):
        raise ctypes.WinError()
    screen_dc = user32.GetDC(0)
    if not screen_dc:
        raise ctypes.WinError()
    memory_dc = gdi32.CreateCompatibleDC(screen_dc)
    bitmap = gdi32.CreateCompatibleBitmap(screen_dc, bounds.width, bounds.height)
    if not memory_dc or not bitmap:
        if bitmap:
            gdi32.DeleteObject(bitmap)
        if memory_dc:
            gdi32.DeleteDC(memory_dc)
        user32.ReleaseDC(0, screen_dc)
        raise ctypes.WinError()

    previous = gdi32.SelectObject(memory_dc, bitmap)
    try:
        if not gdi32.BitBlt(
            memory_dc,
            0,
            0,
            bounds.width,
            bounds.height,
            screen_dc,
            int(rect.left),
            int(rect.top),
            0x00CC0020,
        ):
            raise ctypes.WinError()
        info = records.BitmapInfo()
        info.header = records.BitmapInfoHeader(
            ctypes.sizeof(records.BitmapInfoHeader),
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
        user32.ReleaseDC(0, screen_dc)
