"""Bounded Windows monitor placement and display-scale observations."""

from __future__ import annotations

import ctypes
from contextlib import contextmanager
import sys
from dataclasses import dataclass


@dataclass(frozen=True)
class MonitorBounds:
    """One monitor's desktop and work-area rectangles in physical pixels."""

    index: int
    left: int
    top: int
    right: int
    bottom: int
    work_left: int
    work_top: int
    work_right: int
    work_bottom: int

    @property
    def work_width(self) -> int:
        return self.work_right - self.work_left

    @property
    def work_height(self) -> int:
        return self.work_bottom - self.work_top


class _Rect(ctypes.Structure):
    _fields_ = [
        ("left", ctypes.c_long),
        ("top", ctypes.c_long),
        ("right", ctypes.c_long),
        ("bottom", ctypes.c_long),
    ]


class _MonitorInfo(ctypes.Structure):
    _fields_ = [
        ("size", ctypes.c_uint32),
        ("monitor", _Rect),
        ("work", _Rect),
        ("flags", ctypes.c_uint32),
    ]


@contextmanager
def physical_coordinates():
    """Use physical pixels on this thread and restore its previous DPI context."""
    if sys.platform != "win32":
        raise RuntimeError("native display coordinates require Windows")
    set_context = ctypes.windll.user32.SetThreadDpiAwarenessContext
    set_context.argtypes = [ctypes.c_void_p]
    set_context.restype = ctypes.c_void_p
    # PER_MONITOR_AWARE_V2 prevents GetWindowRect virtualization while GDI
    # captures the target's physical pixels. Worker threads enter separately.
    previous = set_context(ctypes.c_void_p(-4))
    if not previous:
        raise ctypes.WinError()
    try:
        yield
    finally:
        if not set_context(previous):
            raise ctypes.WinError()


@physical_coordinates()
def enumerate_monitors() -> tuple[MonitorBounds, ...]:
    """Return all attached monitors in deterministic desktop-coordinate order."""
    if sys.platform != "win32":
        raise RuntimeError("native monitor enumeration requires Windows")

    user32 = ctypes.windll.user32
    callback_type = getattr(ctypes, "WINFUNCTYPE", ctypes.CFUNCTYPE)(
        ctypes.c_bool,
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.POINTER(_Rect),
        ctypes.c_ssize_t,
    )
    user32.EnumDisplayMonitors.argtypes = [
        ctypes.c_void_p,
        ctypes.c_void_p,
        callback_type,
        ctypes.c_ssize_t,
    ]
    user32.EnumDisplayMonitors.restype = ctypes.c_bool
    user32.GetMonitorInfoW.argtypes = [ctypes.c_void_p, ctypes.POINTER(_MonitorInfo)]
    user32.GetMonitorInfoW.restype = ctypes.c_bool
    handles: list[int] = []

    @callback_type
    def visit(handle: int, _device_context: int, _rect: int, _data: int) -> bool:
        handles.append(int(handle))
        return True

    if not user32.EnumDisplayMonitors(None, None, visit, 0):
        raise ctypes.WinError()

    monitors: list[MonitorBounds] = []
    for handle in handles:
        info = _MonitorInfo(size=ctypes.sizeof(_MonitorInfo))
        if not user32.GetMonitorInfoW(handle, ctypes.byref(info)):
            raise ctypes.WinError()
        monitors.append(
            MonitorBounds(
                index=0,
                left=info.monitor.left,
                top=info.monitor.top,
                right=info.monitor.right,
                bottom=info.monitor.bottom,
                work_left=info.work.left,
                work_top=info.work.top,
                work_right=info.work.right,
                work_bottom=info.work.bottom,
            )
        )
    monitors.sort(key=lambda monitor: (monitor.left, monitor.top, monitor.right, monitor.bottom))
    return tuple(
        MonitorBounds(
            index=index,
            left=monitor.left,
            top=monitor.top,
            right=monitor.right,
            bottom=monitor.bottom,
            work_left=monitor.work_left,
            work_top=monitor.work_top,
            work_right=monitor.work_right,
            work_bottom=monitor.work_bottom,
        )
        for index, monitor in enumerate(monitors)
    )


@physical_coordinates()
def move_window_to_monitor(
    handle: int,
    monitor: MonitorBounds,
    width: int,
    height: int,
) -> None:
    """Center one window in a monitor work area without changing its size."""
    if sys.platform != "win32":
        raise RuntimeError("native monitor placement requires Windows")
    if width <= 0 or height <= 0:
        raise ValueError("window dimensions must be positive")

    left = monitor.work_left + max(0, (monitor.work_width - width) // 2)
    top = monitor.work_top + max(0, (monitor.work_height - height) // 2)
    user32 = ctypes.windll.user32
    user32.SetWindowPos.argtypes = [
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_uint32,
    ]
    user32.SetWindowPos.restype = ctypes.c_bool
    flags = 0x0001 | 0x0004 | 0x0010  # SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE
    if not user32.SetWindowPos(handle, None, left, top, 0, 0, flags):
        raise ctypes.WinError()
    user32.RedrawWindow.argtypes = [
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.c_uint32,
    ]
    user32.RedrawWindow.restype = ctypes.c_bool
    redraw_flags = 0x0001 | 0x0100 | 0x0080  # RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN
    if not user32.RedrawWindow(handle, None, None, redraw_flags):
        raise ctypes.WinError()


def flush_window_queue(handle: int) -> None:
    """Synchronize with the target window thread after a placement message."""
    if sys.platform != "win32":
        raise RuntimeError("native window synchronization requires Windows")
    user32 = ctypes.windll.user32
    user32.SendMessageW.argtypes = [
        ctypes.c_void_p,
        ctypes.c_uint32,
        ctypes.c_size_t,
        ctypes.c_ssize_t,
    ]
    user32.SendMessageW.restype = ctypes.c_ssize_t
    # WM_NULL is synchronous and executes after messages already queued for
    # this HWND, including WM_DPICHANGED.
    user32.SendMessageW(handle, 0x0000, 0, 0)


def display_scale_milli(dpi: int) -> int:
    """Map an integer Windows DPI to Metis's nearest-thousandth scale."""
    if dpi <= 0:
        raise ValueError("DPI must be positive")
    return (dpi * 1_000 + 48) // 96
