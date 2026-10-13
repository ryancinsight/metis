"""Bounded client points and the Win32 absolute virtual-desktop grid."""
from __future__ import annotations
import ctypes
from native_input.windows import abi


MAX_SCROLL_NOTCHES = 100


MAX_CLICK_COORDINATE = 16_384


ABSOLUTE_COORDINATE_MAX = 65_535


def validate_click(x: int, y: int) -> tuple[int, int]:
    for coordinate in (x, y):
        if coordinate < 0 or coordinate > MAX_CLICK_COORDINATE:
            raise ValueError(
                f"--click-client coordinates must lie in 0..{MAX_CLICK_COORDINATE}"
            )
    return x, y


def absolute_point(
    screen_x: int, screen_y: int, virtual: tuple[int, int, int, int]
) -> tuple[int, int]:
    """Normalize a screen point onto the SendInput absolute virtual-desktop grid."""
    origin_x, origin_y, width, height = virtual
    if width <= 0 or height <= 0:
        raise ValueError("virtual desktop must have a positive extent")
    normalized = []
    for coordinate, origin, extent in (
        (screen_x, origin_x, width),
        (screen_y, origin_y, height),
    ):
        value = round((coordinate - origin) * ABSOLUTE_COORDINATE_MAX / max(extent - 1, 1))
        normalized.append(min(max(value, 0), ABSOLUTE_COORDINATE_MAX))
    return normalized[0], normalized[1]


def validate_scroll(x: int, y: int, notches: int) -> tuple[int, int, int]:
    validate_click(x, y)
    if notches == 0 or abs(notches) > MAX_SCROLL_NOTCHES:
        raise ValueError(
            f"--scroll-client notches must be nonzero within +/-{MAX_SCROLL_NOTCHES}"
        )
    return x, y, notches


def pointer_records(
    handle: int, client_x: int, client_y: int
) -> tuple[tuple[int, int], int, int]:
    """Resolve a client point to the absolute SendInput virtual-desktop grid."""
    user32 = ctypes.windll.user32
    point = abi.Point(client_x, client_y)
    user32.ClientToScreen.argtypes = [ctypes.c_void_p, ctypes.POINTER(abi.Point)]
    user32.ClientToScreen.restype = ctypes.c_int
    if not user32.ClientToScreen(handle, ctypes.byref(point)):
        raise ctypes.WinError()
    virtual = (
        user32.GetSystemMetrics(abi.SM_XVIRTUALSCREEN),
        user32.GetSystemMetrics(abi.SM_YVIRTUALSCREEN),
        user32.GetSystemMetrics(abi.SM_CXVIRTUALSCREEN),
        user32.GetSystemMetrics(abi.SM_CYVIRTUALSCREEN),
    )
    absolute_x, absolute_y = absolute_point(point.x, point.y, virtual)
    return (point.x, point.y), absolute_x, absolute_y
