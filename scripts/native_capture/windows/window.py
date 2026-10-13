"""Physical Windows discovery, geometry, resize and orderly close."""
from __future__ import annotations
import ctypes
import sys
from native_capture import limits, model
from native_input.windows import abi
from native_display import physical_coordinates


@physical_coordinates()
def window_for_processes(process_ids: set[int]) -> model.WindowBounds:
    if sys.platform != "win32":
        raise RuntimeError("visible native capture requires Windows")

    user32 = ctypes.windll.user32
    callback_type = ctypes.WINFUNCTYPE(ctypes.c_int32, ctypes.c_void_p, ctypes.c_void_p)
    user32.EnumWindows.argtypes = [callback_type, ctypes.c_void_p]
    user32.GetWindowThreadProcessId.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(ctypes.c_uint32),
    ]
    user32.GetWindowThreadProcessId.restype = ctypes.c_uint32
    user32.IsWindowVisible.argtypes = [ctypes.c_void_p]
    user32.IsWindowVisible.restype = ctypes.c_int32
    user32.GetWindowRect.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(abi.Rect),
    ]
    user32.GetWindowRect.restype = ctypes.c_int32
    found: list[model.WindowBounds] = []

    @callback_type
    def visit(hwnd: int, _lparam: int) -> bool:
        owner = ctypes.c_uint32()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value not in process_ids or not user32.IsWindowVisible(hwnd):
            return True
        rectangle = abi.Rect()
        if not user32.GetWindowRect(hwnd, ctypes.byref(rectangle)):
            return True
        width = rectangle.right - rectangle.left
        height = rectangle.bottom - rectangle.top
        if width > 0 and height > 0:
            found.append(model.WindowBounds(int(hwnd), width, height))
            return False
        return True

    user32.EnumWindows(visit, None)
    if not found:
        raise RuntimeError("visible process window was not discoverable")
    return found[0]


@physical_coordinates()
def window_observation(handle: int) -> model.WindowObservation:
    """Read outer/client geometry and the effective per-window DPI."""
    if sys.platform != "win32":
        raise RuntimeError("visible native capture requires Windows")

    user32 = ctypes.windll.user32
    user32.GetWindowRect.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(abi.Rect),
    ]
    user32.GetWindowRect.restype = ctypes.c_int32
    user32.GetClientRect.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(abi.Rect),
    ]
    user32.GetClientRect.restype = ctypes.c_int32
    user32.GetDpiForWindow.argtypes = [ctypes.c_void_p]
    user32.GetDpiForWindow.restype = ctypes.c_uint32

    outer = abi.Rect()
    if not user32.GetWindowRect(handle, ctypes.byref(outer)):
        raise ctypes.WinError()
    client = abi.Rect()
    if not user32.GetClientRect(handle, ctypes.byref(client)):
        raise ctypes.WinError()
    dpi = int(user32.GetDpiForWindow(handle))
    if dpi == 0:
        raise ctypes.WinError()
    outer_width = outer.right - outer.left
    outer_height = outer.bottom - outer.top
    client_width = client.right - client.left
    client_height = client.bottom - client.top
    if outer_width <= 0 or outer_height <= 0 or client_width <= 0 or client_height <= 0:
        raise RuntimeError("native window reported non-positive geometry")
    return model.WindowObservation(
        bounds=model.WindowBounds(int(handle), outer_width, outer_height),
        client_width=client_width,
        client_height=client_height,
        dpi=dpi,
    )


@physical_coordinates()
def resize_window(handle: int, client_width: int, client_height: int) -> model.WindowObservation:
    """Resize one visible HWND to a validated client size and read it back."""
    if sys.platform != "win32":
        raise RuntimeError("visible native capture requires Windows")
    limits.validate_dimensions(client_width, client_height, "resize")

    user32 = ctypes.windll.user32
    user32.GetWindowLongPtrW.argtypes = [ctypes.c_void_p, ctypes.c_int]
    user32.GetWindowLongPtrW.restype = ctypes.c_ssize_t
    user32.GetMenu.argtypes = [ctypes.c_void_p]
    user32.GetMenu.restype = ctypes.c_void_p
    user32.AdjustWindowRectExForDpi.argtypes = [
        ctypes.POINTER(abi.Rect),
        ctypes.c_uint32,
        ctypes.c_int32,
        ctypes.c_uint32,
        ctypes.c_uint32,
    ]
    user32.AdjustWindowRectExForDpi.restype = ctypes.c_int32
    user32.GetDpiForWindow.argtypes = [ctypes.c_void_p]
    user32.GetDpiForWindow.restype = ctypes.c_uint32
    user32.SetWindowPos.argtypes = [
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_uint32,
    ]
    user32.SetWindowPos.restype = ctypes.c_int32
    user32.RedrawWindow.argtypes = [
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.c_uint32,
    ]
    user32.RedrawWindow.restype = ctypes.c_int32

    style = int(user32.GetWindowLongPtrW(handle, -16)) & 0xFFFFFFFF
    extended_style = int(user32.GetWindowLongPtrW(handle, -20)) & 0xFFFFFFFF
    adjusted = abi.Rect(0, 0, client_width, client_height)
    dpi = user32.GetDpiForWindow(handle)
    if dpi == 0:
        raise ctypes.WinError()
    if not user32.AdjustWindowRectExForDpi(
        ctypes.byref(adjusted),
        style,
        bool(user32.GetMenu(handle)),
        extended_style,
        dpi,
    ):
        raise ctypes.WinError()
    outer_width = adjusted.right - adjusted.left
    outer_height = adjusted.bottom - adjusted.top
    if outer_width <= 0 or outer_height <= 0:
        raise RuntimeError("window style produced non-positive resize geometry")
    flags = 0x0002 | 0x0004 | 0x0010  # SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE
    if not user32.SetWindowPos(
        handle,
        None,
        0,
        0,
        outer_width,
        outer_height,
        flags,
    ):
        raise ctypes.WinError()
    redraw_flags = 0x0001 | 0x0100 | 0x0080  # RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN
    if not user32.RedrawWindow(handle, None, None, redraw_flags):
        raise ctypes.WinError()
    observation = window_observation(handle)
    if (observation.client_width, observation.client_height) != (client_width, client_height):
        raise RuntimeError(
            "native window did not apply the requested client size: "
            f"requested {client_width}x{client_height}, "
            f"observed {observation.client_width}x{observation.client_height}"
        )
    return observation


def close_window(handle: int) -> None:
    """Request orderly close of a captured native window."""
    user32 = ctypes.windll.user32
    user32.PostMessageW.argtypes = [
        ctypes.c_void_p,
        ctypes.c_uint32,
        ctypes.c_size_t,
        ctypes.c_ssize_t,
    ]
    user32.PostMessageW.restype = ctypes.c_int32
    if not user32.PostMessageW(handle, 0x0010, 0, 0):
        raise ctypes.WinError()
