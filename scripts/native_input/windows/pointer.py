"""Visible-surface admission and Win32 pointer/wheel delivery."""
from __future__ import annotations
import ctypes
import time
from native_capture.windows import window
from native_input.windows import abi, focus, geometry


WHEEL_NOTCH_WAIT_SECONDS = 0.02


def coverer(handle: int, point_x: int, point_y: int) -> tuple[int, str] | None:
    """The foreign top-level window covering a screen point, if any.

    Returns that window's handle and class name so callers can name the
    obstruction in their refusal instead of failing without cause.
    """
    user32 = ctypes.windll.user32
    user32.WindowFromPoint.argtypes = [abi.Point]
    user32.WindowFromPoint.restype = ctypes.c_void_p
    user32.GetAncestor.argtypes = [ctypes.c_void_p, ctypes.c_uint]
    user32.GetAncestor.restype = ctypes.c_void_p
    hit = user32.WindowFromPoint(abi.Point(point_x, point_y))
    hit_root = user32.GetAncestor(hit, abi.GA_ROOT) if hit else 0
    target_root = user32.GetAncestor(handle, abi.GA_ROOT) or handle
    if hit_root == target_root:
        return None
    covering_handle = int(hit_root or hit or 0)
    return covering_handle, focus.window_class(covering_handle) if covering_handle else ""


def guard_surface_visible(handle: int) -> None:
    """Refuse a screen capture whose surface another window covers.

    Screen BitBlt reads whatever is visible at the window's screen rect, so
    a covering window corrupts every recorded frame even when the capture
    itself succeeds. Five interior samples spread over the surface stand
    for the whole.

    # Errors
    Raises `RuntimeError` naming the covering window when any sample point
    resolves outside the target's top-level hierarchy.
    """
    observation = window.window_observation(handle)
    width = observation.client_width
    height = observation.client_height
    samples = (
        (width // 4, height // 4),
        (3 * width // 4, height // 4),
        (width // 2, height // 2),
        (width // 4, 3 * height // 4),
        (3 * width // 4, 3 * height // 4),
    )
    for client_x, client_y in samples:
        (point_x, point_y), _, _ = geometry.pointer_records(handle, client_x, client_y)
        covering_window = coverer(handle, point_x, point_y)
        if covering_window is not None:
            covering_handle, class_name = covering_window
            raise RuntimeError(
                f"another window ({class_name or 'unknown class'} {hex(covering_handle)}) "
                f"covers the capture surface at ({point_x}, {point_y}); "
                "dismiss or move it before recording a screen capture"
            )


def wheel_data(notches: int) -> int:
    """The signed wheel delta as the unsigned `MOUSEINPUT.mouseData` bits."""
    return (notches * abi.WHEEL_DELTA) & 0xFFFFFFFF


def guard_input_point(handle: int, point_x: int, point_y: int) -> None:
    """Refuse a pointer claim when another window owns the input point.

    Trusted OS pointer input goes to the window under the cursor, so a
    topmost overlay covering the point receives the click and wheel while
    a screen capture still shows the target through the overlay.

    # Errors
    Raises `RuntimeError` when the input point resolves to a window
    outside the target's top-level hierarchy.
    """
    covering_window = coverer(handle, point_x, point_y)
    if covering_window is not None:
        covering_handle, class_name = covering_window
        raise RuntimeError(
            f"another window ({class_name or 'unknown class'} {hex(covering_handle)}) "
            f"covers the trusted input point ({point_x}, {point_y}); "
            "no pointer claim can be recorded"
        )


def send_wheel(handle: int, client_x: int, client_y: int, notches: int) -> None:
    """Send trusted OS wheel notches over one client point."""
    user32 = ctypes.windll.user32
    (point_x, point_y), absolute_x, absolute_y = geometry.pointer_records(
        handle, client_x, client_y
    )
    guard_input_point(handle, point_x, point_y)
    user32.SendInput.argtypes = [
        ctypes.c_uint,
        ctypes.POINTER(abi.Input),
        ctypes.c_int,
    ]
    user32.SendInput.restype = ctypes.c_uint
    move = abi.Input(kind=abi.SEND_INPUT_MOUSE)
    move.mouse = abi.MouseInput(
        dx=absolute_x,
        dy=absolute_y,
        mouse_data=0,
        flags=abi.MOUSEEVENTF_ABSOLUTE | abi.MOUSEEVENTF_VIRTUALDESK | abi.MOUSEEVENTF_MOVE,
        time=0,
        extra_info=0,
    )
    direction = 1 if notches > 0 else -1
    # One notch per record: a burst of wheel records is coalesced by the
    # browser and only part of it reaches the page. The wheel record
    # carries no movement either: `dx` and `dy` without
    # `abi.MOUSEEVENTF_ABSOLUTE` are relative deltas that would drag the
    # cursor (and the wheel) away from the claimed point.
    for _ in range(abs(notches)):
        wheel = abi.Input(kind=abi.SEND_INPUT_MOUSE)
        wheel.mouse = abi.MouseInput(
            dx=0,
            dy=0,
            mouse_data=wheel_data(direction),
            flags=abi.MOUSEEVENTF_WHEEL,
            time=0,
            extra_info=0,
        )
        records = (abi.Input * 2)(move, wheel)
        sent = user32.SendInput(2, records, ctypes.sizeof(abi.Input))
        if sent != 2:
            raise ctypes.WinError()
        time.sleep(WHEEL_NOTCH_WAIT_SECONDS)


def click_records(absolute_x: int, absolute_y: int) -> tuple[abi.Input, abi.Input, abi.Input]:
    """Move, press and release records for one trusted click.

    The move record owns the positioning. Press and release carry no
    movement: without `MOUSEEVENTF_ABSOLUTE`, `dx` and `dy` are relative
    deltas, so reusing the absolute grid coordinates there would drag the
    cursor thousands of pixels and deliver the button events away from the
    claimed point.
    """
    move = abi.Input(kind=abi.SEND_INPUT_MOUSE)
    move.mouse = abi.MouseInput(
        dx=absolute_x,
        dy=absolute_y,
        mouse_data=0,
        flags=abi.MOUSEEVENTF_ABSOLUTE | abi.MOUSEEVENTF_VIRTUALDESK | abi.MOUSEEVENTF_MOVE,
        time=0,
        extra_info=0,
    )
    press = abi.Input(kind=abi.SEND_INPUT_MOUSE)
    press.mouse = abi.MouseInput(
        dx=0,
        dy=0,
        mouse_data=0,
        flags=abi.MOUSEEVENTF_LEFTDOWN,
        time=0,
        extra_info=0,
    )
    release = abi.Input(kind=abi.SEND_INPUT_MOUSE)
    release.mouse = abi.MouseInput(
        dx=0,
        dy=0,
        mouse_data=0,
        flags=abi.MOUSEEVENTF_LEFTUP,
        time=0,
        extra_info=0,
    )
    return move, press, release


def send_pointer_click(handle: int, client_x: int, client_y: int) -> tuple[int, int]:
    """Send one trusted OS left click at a client point; return its screen point."""
    user32 = ctypes.windll.user32
    (point_x, point_y), absolute_x, absolute_y = geometry.pointer_records(
        handle, client_x, client_y
    )
    guard_input_point(handle, point_x, point_y)
    user32.SendInput.argtypes = [
        ctypes.c_uint,
        ctypes.POINTER(abi.Input),
        ctypes.c_int,
    ]
    user32.SendInput.restype = ctypes.c_uint
    records = (abi.Input * 3)(*click_records(absolute_x, absolute_y))
    sent = user32.SendInput(3, records, ctypes.sizeof(abi.Input))
    if sent != 3:
        raise ctypes.WinError()
    return point_x, point_y
