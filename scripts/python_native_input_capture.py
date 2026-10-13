"""Capture a production native window before and after trusted Win32 input."""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import pathlib
import struct
import subprocess
import sys
import time
from typing import Any

MAX_TEXT_BYTES = 128
SETTLE_ROUNDS = 100
SETTLE_WAIT_SECONDS = 0.1
STABLE_ROUNDS = 5
SETTLE_HISTORY = 4
POST_INPUT_MIN_OBSERVE_SECONDS = 5.0
FOCUS_ROUNDS = 20
FOCUS_WAIT_SECONDS = 0.1

import python_native_capture as capture


SEND_INPUT_MOUSE = 0
SEND_INPUT_KEYBOARD = 1
VK_CONTROL = 0x11
VK_RETURN = 0x0D
VK_MENU = 0x12
KEYEVENTF_KEYUP = 0x0002
KEYEVENTF_UNICODE = 0x0004
MOUSEEVENTF_LEFTDOWN = 0x0002
MOUSEEVENTF_LEFTUP = 0x0004
MOUSEEVENTF_WHEEL = 0x0800
MOUSEEVENTF_ABSOLUTE = 0x8000
MOUSEEVENTF_VIRTUALDESK = 0x0100
WHEEL_DELTA = 120
MAX_SCROLL_NOTCHES = 100
WHEEL_NOTCH_WAIT_SECONDS = 0.02
SM_XVIRTUALSCREEN = 76
SM_YVIRTUALSCREEN = 77
SM_CXVIRTUALSCREEN = 78
SM_CYVIRTUALSCREEN = 79
MAX_CLICK_COORDINATE = 16_384
ABSOLUTE_COORDINATE_MAX = 65_535
WM_NULL = 0x0000
SMTO_ABORT_IF_HUNG = 0x0002
SMTO_BLOCK = 0x0001
INPUT_FLUSH_TIMEOUT_MILLISECONDS = 1_000
HWND_TOPMOST = ctypes.c_void_p(-1 & 0xFFFFFFFFFFFFFFFF)
GA_ROOT = 3
SWP_NOSIZE = 0x0001
SWP_NOMOVE = 0x0002
SWP_NOACTIVATE = 0x0010


class _KeyboardInput(ctypes.Structure):
    _fields_ = [
        ("virtual_key", ctypes.c_ushort),
        ("scan_code", ctypes.c_ushort),
        ("flags", ctypes.c_uint32),
        ("time", ctypes.c_uint32),
        ("extra_info", ctypes.c_size_t),
    ]


class _MouseInput(ctypes.Structure):
    _fields_ = [
        ("dx", ctypes.c_long),
        ("dy", ctypes.c_long),
        ("mouse_data", ctypes.c_uint32),
        ("flags", ctypes.c_uint32),
        ("time", ctypes.c_uint32),
        ("extra_info", ctypes.c_size_t),
    ]


class _Point(ctypes.Structure):
    _fields_ = [("x", ctypes.c_long), ("y", ctypes.c_long)]


class _InputUnion(ctypes.Union):
    # INPUT is a tagged union whose MOUSEINPUT arm is wider than KEYBDINPUT
    # on 64-bit Windows. Retain that ABI width across both arms.
    _fields_ = [
        ("keyboard", _KeyboardInput),
        ("mouse", _MouseInput),
        ("abi_padding", ctypes.c_ubyte * 32),
    ]


class _Input(ctypes.Structure):
    _anonymous_ = ("value",)
    _fields_ = [("kind", ctypes.c_uint32), ("value", _InputUnion)]


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Capture a production Windows window before and after Unicode input."
    )
    parser.add_argument("--command", type=pathlib.Path, required=True)
    parser.add_argument(
        "--argument",
        action="append",
        default=[],
        dest="command_arguments",
        help="one argument passed to --command; repeat for each argument",
    )
    parser.add_argument("--cwd", type=pathlib.Path)
    parser.add_argument(
        "--text",
        help="bounded Unicode text delivered through Win32 SendInput",
    )
    parser.add_argument(
        "--shortcut",
        choices=("enter", "control-enter"),
        help="optional submit shortcut delivered after the Unicode text",
    )
    parser.add_argument(
        "--click-client",
        nargs=2,
        type=int,
        metavar=("X", "Y"),
        dest="click_client",
        help="trusted OS pointer click at this 96-dpi logical client point",
    )
    parser.add_argument(
        "--scroll-client",
        nargs=3,
        type=int,
        metavar=("X", "Y", "NOTCHES"),
        dest="scroll_client",
        help="trusted OS wheel notches over this 96-dpi logical client point",
    )
    parser.add_argument(
        "--screen",
        action="store_true",
        help="read visible screen pixels for composition surfaces instead of WM_PRINT",
    )
    parser.add_argument("--initial-output", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--manifest", type=pathlib.Path, required=True)
    return parser


def _validate_text(text: str) -> tuple[bytes, tuple[int, ...]]:
    encoded = text.encode("utf-8")
    if not text:
        raise ValueError("--text must not be empty")
    if len(encoded) > MAX_TEXT_BYTES:
        raise ValueError(f"--text exceeds the {MAX_TEXT_BYTES}-byte bound")
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in text):
        raise ValueError("--text contains a control character")
    utf16 = text.encode("utf-16-le", "strict")
    units = struct.unpack(f"<{len(utf16) // 2}H", utf16)
    return encoded, units


def _validate_paths(
    command: pathlib.Path,
    initial_output: pathlib.Path,
    output: pathlib.Path,
    manifest: pathlib.Path,
) -> pathlib.Path:
    executable = command.resolve(strict=True)
    if not executable.is_file():
        raise ValueError("--command must name a file")
    resolved = [initial_output.resolve(), output.resolve(), manifest.resolve()]
    if len(set(resolved)) != len(resolved):
        raise ValueError("capture outputs must be distinct")
    if initial_output.resolve() == output.resolve():
        raise ValueError("--initial-output must differ from --output")
    if initial_output.suffix.lower() not in {".bmp", ".png"}:
        raise ValueError("--initial-output must end in .bmp or .png")
    if output.suffix.lower() not in {".bmp", ".png"}:
        raise ValueError("--output must end in .bmp or .png")
    if manifest.suffix.lower() != ".json":
        raise ValueError("--manifest must end in .json")
    return executable


def _window_class(handle: int) -> str:
    """The Win32 class name of `handle`, empty when it is not a window."""
    user32 = ctypes.windll.user32
    user32.GetClassNameW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]
    user32.GetClassNameW.restype = ctypes.c_int
    name = ctypes.create_unicode_buffer(64)
    user32.GetClassNameW(handle, name, 64)
    return name.value


def _pick_content_child(
    candidates: tuple[tuple[int, int], ...], client_area: int
) -> int | None:
    """The hosted content child among `candidates`, if one exists.

    Each candidate pairs a visible descendant window handle with its window
    area. The surface a host embeds (a browser render child) covers at
    least half of the client area, while a window that paints its own
    content has no qualifying descendant. The largest qualifying candidate
    wins.

    Returns `None` when the client area is degenerate or no candidate is
    large enough to be the embedded content surface.
    """
    if client_area <= 0:
        return None
    qualifying = [item for item in candidates if item[1] * 2 >= client_area]
    if not qualifying:
        return None
    return max(qualifying, key=lambda item: item[1])[0]


def _content_child(handle: int) -> int | None:
    """The handle's hosted content child (see [`_pick_content_child`])."""
    user32 = ctypes.windll.user32
    user32.EnumChildWindows.argtypes = [
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.c_void_p,
    ]
    user32.EnumChildWindows.restype = ctypes.c_bool
    user32.IsWindowVisible.argtypes = [ctypes.c_void_p]
    user32.IsWindowVisible.restype = ctypes.c_bool
    user32.GetClientRect.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(capture._WindowRect),
    ]
    user32.GetClientRect.restype = ctypes.c_bool
    candidates: list[tuple[int, int]] = []

    @ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)
    def collect(child, _parameter):
        if not user32.IsWindowVisible(child):
            return True
        rect = capture._WindowRect()
        if user32.GetClientRect(child, ctypes.byref(rect)):
            candidates.append(
                (
                    child,
                    max(0, rect.right - rect.left) * max(0, rect.bottom - rect.top),
                )
            )
        return True

    user32.EnumChildWindows(handle, collect, None)
    client = capture._WindowRect()
    if not user32.GetClientRect(handle, ctypes.byref(client)):
        return None
    client_area = max(0, client.right - client.left) * max(0, client.bottom - client.top)
    return _pick_content_child(tuple(candidates), client_area)


def _coverer(handle: int, point_x: int, point_y: int) -> tuple[int, str] | None:
    """The foreign top-level window covering a screen point, if any.

    Returns that window's handle and class name so callers can name the
    obstruction in their refusal instead of failing without cause.
    """
    user32 = ctypes.windll.user32
    user32.WindowFromPoint.argtypes = [_Point]
    user32.WindowFromPoint.restype = ctypes.c_void_p
    user32.GetAncestor.argtypes = [ctypes.c_void_p, ctypes.c_uint]
    user32.GetAncestor.restype = ctypes.c_void_p
    hit = user32.WindowFromPoint(_Point(point_x, point_y))
    hit_root = user32.GetAncestor(hit, GA_ROOT) if hit else 0
    target_root = user32.GetAncestor(handle, GA_ROOT) or handle
    if hit_root == target_root:
        return None
    coverer = int(hit_root or hit or 0)
    return coverer, _window_class(coverer) if coverer else ""


def _guard_surface_visible(handle: int) -> None:
    """Refuse a screen capture whose surface another window covers.

    Screen BitBlt reads whatever is visible at the window's screen rect, so
    a covering window corrupts every recorded frame even when the capture
    itself succeeds. Five interior samples spread over the surface stand
    for the whole.

    # Errors
    Raises `RuntimeError` naming the covering window when any sample point
    resolves outside the target's top-level hierarchy.
    """
    observation = capture._window_observation(handle)
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
        (point_x, point_y), _, _ = _pointer_records(handle, client_x, client_y)
        coverer = _coverer(handle, point_x, point_y)
        if coverer is not None:
            window, class_name = coverer
            raise RuntimeError(
                f"another window ({class_name or 'unknown class'} {hex(window)}) "
                f"covers the capture surface at ({point_x}, {point_y}); "
                "dismiss or move it before recording a screen capture"
            )


def _focus_window(handle: int) -> tuple[int, bool, int]:
    """Give `handle`'s keyboard target focus and verify the activation.

    Returns the keyboard layout of the target thread, whether the window is
    the foreground window, and the window that received keyboard focus. A
    hosted content child (a browser render surface) owns the keyboard, so
    focusing only the top-level window leaves typed text and navigation
    keys in the host's own focus chain instead of the page.

    # Errors
    Raises `RuntimeError` when the keyboard focus cannot be placed on the
    target or a window state read fails.
    """
    user32 = ctypes.windll.user32
    kernel32 = ctypes.windll.kernel32
    user32.BringWindowToTop.argtypes = [ctypes.c_void_p]
    user32.BringWindowToTop.restype = ctypes.c_bool
    user32.GetWindowThreadProcessId.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(ctypes.c_uint32),
    ]
    user32.GetWindowThreadProcessId.restype = ctypes.c_uint32
    user32.AttachThreadInput.argtypes = [ctypes.c_uint32, ctypes.c_uint32, ctypes.c_bool]
    user32.AttachThreadInput.restype = ctypes.c_bool
    user32.SetForegroundWindow.argtypes = [ctypes.c_void_p]
    user32.SetForegroundWindow.restype = ctypes.c_bool
    user32.SetFocus.argtypes = [ctypes.c_void_p]
    user32.SetFocus.restype = ctypes.c_void_p
    user32.GetFocus.restype = ctypes.c_void_p
    user32.GetForegroundWindow.argtypes = []
    user32.GetForegroundWindow.restype = ctypes.c_void_p
    user32.SetWindowPos.argtypes = [
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_uint,
    ]
    user32.SetWindowPos.restype = ctypes.c_bool
    kernel32.GetCurrentThreadId.restype = ctypes.c_uint32
    user32.GetKeyboardLayout.argtypes = [ctypes.c_uint32]
    user32.GetKeyboardLayout.restype = ctypes.c_void_p

    # A hosted content child (a browser render surface) owns the keyboard;
    # keyboard input must target it instead of the top-level host window.
    focus_target = _content_child(handle) or handle
    process_id = ctypes.c_uint32()
    target_thread = user32.GetWindowThreadProcessId(handle, ctypes.byref(process_id))
    if target_thread == 0:
        raise ctypes.WinError()
    current_thread = kernel32.GetCurrentThreadId()
    attached = current_thread != target_thread
    if attached and not user32.AttachThreadInput(current_thread, target_thread, True):
        raise ctypes.WinError()
    # The activation right lives with the current foreground thread, so
    # attach to its queue as well; an attached thread may move the
    # foreground where a plain request is refused.
    foreground_handle = user32.GetForegroundWindow()
    foreground_thread = (
        user32.GetWindowThreadProcessId(foreground_handle, None)
        if foreground_handle
        else 0
    )
    foreground_attached = (
        foreground_thread not in (0, current_thread, target_thread)
        and user32.AttachThreadInput(current_thread, foreground_thread, True)
    )
    detach_failed = False
    try:
        # Trusted input is delivered by hit test and foreground, and a
        # topmost system overlay (for example the shell's screen dimmer)
        # would keep both while a screen capture still shows the target
        # through it; pin the target above such overlays for the recording.
        user32.SetWindowPos(
            handle,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
        user32.BringWindowToTop(handle)
        # Foreground activation can lose the race with the window's own
        # creation, and a denied activation makes Windows consume the next
        # click on the window instead of delivering it. A bare ALT tap
        # through the trusted input queue grants the activation right
        # without changing any application state.
        foreground_set = bool(user32.SetForegroundWindow(handle))
        user32.SetFocus(focus_target)
        if not foreground_set:
            _send_virtual_key(VK_MENU, key_up=False)
            _send_virtual_key(VK_MENU, key_up=True)
            foreground_set = bool(user32.SetForegroundWindow(handle))
            user32.SetFocus(focus_target)
        focus_rounds = 0
        while (
            focus_rounds < FOCUS_ROUNDS
            and not (
                foreground_set and int(user32.GetFocus() or 0) == focus_target
            )
        ):
            focus_rounds += 1
            time.sleep(FOCUS_WAIT_SECONDS)
            # The activation right is consumed by whichever window moves
            # first, so one ALT tap can lose the race; repeat the trusted
            # tap with each bounded retry.
            _send_virtual_key(VK_MENU, key_up=False)
            _send_virtual_key(VK_MENU, key_up=True)
            foreground_set = bool(user32.SetForegroundWindow(handle))
            user32.SetFocus(focus_target)
        # Judge activation by the actual foreground window; the
        # SetForegroundWindow return value alone is not authoritative.
        foreground_set = int(user32.GetForegroundWindow() or 0) == handle
        if int(user32.GetFocus() or 0) != focus_target:
            raise RuntimeError(
                f"the window {hex(focus_target)} could not receive keyboard focus "
                f"(foreground_set={foreground_set})"
            )
    finally:
        # A detach failure must not mask an error the try block is
        # already raising, so record it and surface it only on a clean
        # exit.
        if foreground_attached and not user32.AttachThreadInput(
            current_thread, foreground_thread, False
        ):
            detach_failed = True
        if attached and not user32.AttachThreadInput(current_thread, target_thread, False):
            detach_failed = True
    if detach_failed:
        raise ctypes.WinError()
    return (
        int(user32.GetKeyboardLayout(target_thread)),
        foreground_set,
        focus_target,
    )


def _require_foreground(handle: int) -> None:
    """Re-verify foreground activation immediately before an input claim.

    Activation can be lost during the bounded settle observations that run
    between focusing and delivery: Windows then consumes the first click on
    the target for activation instead of delivering it, and routes wheel
    and keyboard input to the new foreground window, so the claim would
    record input that never reached the target.

    # Errors
    Raises `RuntimeError` when activation cannot be re-established, which
    is a failed capture under the manifest contract.
    """
    user32 = ctypes.windll.user32
    user32.GetForegroundWindow.argtypes = []
    user32.GetForegroundWindow.restype = ctypes.c_void_p
    if int(user32.GetForegroundWindow() or 0) == handle:
        return
    _, foreground_set, _ = _focus_window(handle)
    if not foreground_set:
        raise RuntimeError(
            "Windows denied foreground activation at input delivery; "
            "no trusted input claim can be recorded"
        )


def _send_unicode_text(text_units: tuple[int, ...]) -> None:
    user32 = ctypes.windll.user32
    user32.SendInput.argtypes = [
        ctypes.c_uint,
        ctypes.POINTER(_Input),
        ctypes.c_int,
    ]
    user32.SendInput.restype = ctypes.c_uint
    inputs = (_Input * (2 * len(text_units)))()
    for index, unit in enumerate(text_units):
        inputs[2 * index].kind = SEND_INPUT_KEYBOARD
        inputs[2 * index].keyboard = _KeyboardInput(
            virtual_key=0,
            scan_code=unit,
            flags=KEYEVENTF_UNICODE,
            time=0,
            extra_info=0,
        )
        inputs[2 * index + 1].kind = SEND_INPUT_KEYBOARD
        inputs[2 * index + 1].keyboard = _KeyboardInput(
            virtual_key=0,
            scan_code=unit,
            flags=KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
            time=0,
            extra_info=0,
        )
    sent = user32.SendInput(len(inputs), inputs, ctypes.sizeof(_Input))
    if sent != len(inputs):
        raise ctypes.WinError()


def _send_virtual_key(virtual_key: int, *, key_up: bool) -> None:
    user32 = ctypes.windll.user32
    user32.SendInput.argtypes = [
        ctypes.c_uint,
        ctypes.POINTER(_Input),
        ctypes.c_int,
    ]
    user32.SendInput.restype = ctypes.c_uint
    record = _Input(kind=SEND_INPUT_KEYBOARD)
    record.keyboard = _KeyboardInput(
        virtual_key=virtual_key,
        scan_code=0,
        flags=KEYEVENTF_KEYUP if key_up else 0,
        time=0,
        extra_info=0,
    )
    sent = user32.SendInput(1, ctypes.byref(record), ctypes.sizeof(_Input))
    if sent != 1:
        raise ctypes.WinError()


def _send_shortcut(shortcut: str) -> None:
    if shortcut == "enter":
        _send_virtual_key(VK_RETURN, key_up=False)
        _send_virtual_key(VK_RETURN, key_up=True)
        return
    if shortcut == "control-enter":
        _send_virtual_key(VK_CONTROL, key_up=False)
        try:
            _send_virtual_key(VK_RETURN, key_up=False)
            _send_virtual_key(VK_RETURN, key_up=True)
        finally:
            _send_virtual_key(VK_CONTROL, key_up=True)
        return
    raise ValueError(f"unsupported native shortcut: {shortcut!r}")


def _after_settled(
    *,
    changed: bool,
    repeated: bool,
    observed_seconds: float,
    minimum_seconds: float,
) -> bool:
    """Whether the post-input observation may end.

    The after frame must show the input's response, which may render after
    the input queue drains, so a frame that merely holds still is not
    evidence: the observation ends when a frame that changed from the state
    the input left behind also repeats among the recent frames (the
    response arrived and now holds or cycles, as a blinking caret does),
    or, once the minimum observation window has passed, when the surface
    repeats with nothing new arriving.
    """
    if not repeated:
        return False
    return changed or observed_seconds >= minimum_seconds


def _blank_frame(pixels: bytes, width: int, height: int) -> bool:
    """A frame whose center half is uniform carries no rendered client yet.

    The outer window frame includes title bar, borders and desktop shadow,
    so readiness reads the central region that lies inside the client area
    of every recorded host instead of the chrome around it.
    """
    if width < 4 or height < 4 or len(pixels) < width * height * 4:
        return True
    stride = width * 4
    x0, x1 = width // 4, width - width // 4
    y0, y1 = height // 4, height - height // 4
    reference = pixels[y0 * stride + x0 * 4 : y0 * stride + x0 * 4 + 4]
    for y in range(y0, y1):
        row = pixels[y * stride : (y + 1) * stride]
        for x in range(x0, x1):
            if row[x * 4 : x * 4 + 4] != reference:
                return False
    return True


def _validate_click(x: int, y: int) -> tuple[int, int]:
    for coordinate in (x, y):
        if coordinate < 0 or coordinate > MAX_CLICK_COORDINATE:
            raise ValueError(
                f"--click-client coordinates must lie in 0..{MAX_CLICK_COORDINATE}"
            )
    return x, y


def _absolute_point(
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


def _validate_scroll(x: int, y: int, notches: int) -> tuple[int, int, int]:
    _validate_click(x, y)
    if notches == 0 or abs(notches) > MAX_SCROLL_NOTCHES:
        raise ValueError(
            f"--scroll-client notches must be nonzero within +/-{MAX_SCROLL_NOTCHES}"
        )
    return x, y, notches


def _pointer_records(
    handle: int, client_x: int, client_y: int
) -> tuple[tuple[int, int], int, int]:
    """Resolve a client point to the absolute SendInput virtual-desktop grid."""
    user32 = ctypes.windll.user32
    point = _Point(client_x, client_y)
    user32.ClientToScreen.argtypes = [ctypes.c_void_p, ctypes.POINTER(_Point)]
    user32.ClientToScreen.restype = ctypes.c_int
    if not user32.ClientToScreen(handle, ctypes.byref(point)):
        raise ctypes.WinError()
    virtual = (
        user32.GetSystemMetrics(SM_XVIRTUALSCREEN),
        user32.GetSystemMetrics(SM_YVIRTUALSCREEN),
        user32.GetSystemMetrics(SM_CXVIRTUALSCREEN),
        user32.GetSystemMetrics(SM_CYVIRTUALSCREEN),
    )
    absolute_x, absolute_y = _absolute_point(point.x, point.y, virtual)
    return (point.x, point.y), absolute_x, absolute_y


def _wheel_data(notches: int) -> int:
    """The signed wheel delta as the unsigned `MOUSEINPUT.mouseData` bits."""
    return (notches * WHEEL_DELTA) & 0xFFFFFFFF


def _guard_input_point(handle: int, point_x: int, point_y: int) -> None:
    """Refuse a pointer claim when another window owns the input point.

    Trusted OS pointer input goes to the window under the cursor, so a
    topmost overlay covering the point receives the click and wheel while
    a screen capture still shows the target through the overlay.

    # Errors
    Raises `RuntimeError` when the input point resolves to a window
    outside the target's top-level hierarchy.
    """
    user32 = ctypes.windll.user32
    user32.WindowFromPoint.argtypes = [_Point]
    user32.WindowFromPoint.restype = ctypes.c_void_p
    coverer = _coverer(handle, point_x, point_y)
    if coverer is not None:
        window, class_name = coverer
        raise RuntimeError(
            f"another window ({class_name or 'unknown class'} {hex(window)}) "
            f"covers the trusted input point ({point_x}, {point_y}); "
            "no pointer claim can be recorded"
        )


def _send_wheel(handle: int, client_x: int, client_y: int, notches: int) -> None:
    """Send trusted OS wheel notches over one client point."""
    user32 = ctypes.windll.user32
    (point_x, point_y), absolute_x, absolute_y = _pointer_records(
        handle, client_x, client_y
    )
    _guard_input_point(handle, point_x, point_y)
    user32.SendInput.argtypes = [
        ctypes.c_uint,
        ctypes.POINTER(_Input),
        ctypes.c_int,
    ]
    user32.SendInput.restype = ctypes.c_uint
    move = _Input(kind=SEND_INPUT_MOUSE)
    move.mouse = _MouseInput(
        dx=absolute_x,
        dy=absolute_y,
        mouse_data=0,
        flags=MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK | 0x0001,
        time=0,
        extra_info=0,
    )
    direction = 1 if notches > 0 else -1
    # One notch per record: a burst of wheel records is coalesced by the
    # browser and only part of it reaches the page. The wheel record
    # carries no movement either: `dx` and `dy` without
    # `MOUSEEVENTF_ABSOLUTE` are relative deltas that would drag the
    # cursor (and the wheel) away from the claimed point.
    for _ in range(abs(notches)):
        wheel = _Input(kind=SEND_INPUT_MOUSE)
        wheel.mouse = _MouseInput(
            dx=0,
            dy=0,
            mouse_data=_wheel_data(direction),
            flags=MOUSEEVENTF_WHEEL,
            time=0,
            extra_info=0,
        )
        records = (_Input * 2)(move, wheel)
        sent = user32.SendInput(2, records, ctypes.sizeof(_Input))
        if sent != 2:
            raise ctypes.WinError()
        time.sleep(WHEEL_NOTCH_WAIT_SECONDS)


def _click_records(absolute_x: int, absolute_y: int) -> tuple[_Input, _Input, _Input]:
    """Move, press and release records for one trusted click.

    The move record owns the positioning. Press and release carry no
    movement: without `MOUSEEVENTF_ABSOLUTE`, `dx` and `dy` are relative
    deltas, so reusing the absolute grid coordinates there would drag the
    cursor thousands of pixels and deliver the button events away from the
    claimed point.
    """
    move = _Input(kind=SEND_INPUT_MOUSE)
    move.mouse = _MouseInput(
        dx=absolute_x,
        dy=absolute_y,
        mouse_data=0,
        flags=MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK | 0x0001,
        time=0,
        extra_info=0,
    )
    press = _Input(kind=SEND_INPUT_MOUSE)
    press.mouse = _MouseInput(
        dx=0,
        dy=0,
        mouse_data=0,
        flags=MOUSEEVENTF_LEFTDOWN,
        time=0,
        extra_info=0,
    )
    release = _Input(kind=SEND_INPUT_MOUSE)
    release.mouse = _MouseInput(
        dx=0,
        dy=0,
        mouse_data=0,
        flags=MOUSEEVENTF_LEFTUP,
        time=0,
        extra_info=0,
    )
    return move, press, release


def _send_pointer_click(handle: int, client_x: int, client_y: int) -> tuple[int, int]:
    """Send one trusted OS left click at a client point; return its screen point."""
    user32 = ctypes.windll.user32
    (point_x, point_y), absolute_x, absolute_y = _pointer_records(
        handle, client_x, client_y
    )
    _guard_input_point(handle, point_x, point_y)
    user32.SendInput.argtypes = [
        ctypes.c_uint,
        ctypes.POINTER(_Input),
        ctypes.c_int,
    ]
    user32.SendInput.restype = ctypes.c_uint
    records = (_Input * 3)(*_click_records(absolute_x, absolute_y))
    sent = user32.SendInput(3, records, ctypes.sizeof(_Input))
    if sent != 3:
        raise ctypes.WinError()
    return point_x, point_y


def _flush_window(handle: int) -> None:
    user32 = ctypes.windll.user32
    user32.SendMessageTimeoutW.argtypes = [
        ctypes.c_void_p,
        ctypes.c_uint,
        ctypes.c_size_t,
        ctypes.c_ssize_t,
        ctypes.c_uint,
        ctypes.c_uint,
        ctypes.POINTER(ctypes.c_size_t),
    ]
    user32.SendMessageTimeoutW.restype = ctypes.c_size_t
    result = ctypes.c_size_t()
    if not user32.SendMessageTimeoutW(
        handle,
        WM_NULL,
        0,
        0,
        SMTO_ABORT_IF_HUNG | SMTO_BLOCK,
        INPUT_FLUSH_TIMEOUT_MILLISECONDS,
        ctypes.byref(result),
    ):
        raise ctypes.WinError()


def _record(
    observation: Any,
    image: pathlib.Path,
    digest: str,
) -> dict[str, Any]:
    return {
        "window": {
            "outer": [observation.bounds.width, observation.bounds.height],
            "client": [observation.client_width, observation.client_height],
            "dpi": observation.dpi,
        },
        "image": image.as_posix(),
        "sha256": digest,
    }


def _capture_input(
    command: pathlib.Path,
    command_arguments: list[str],
    cwd: pathlib.Path | None,
    text: str | None,
    shortcut: str | None,
    click: tuple[int, int] | None,
    scroll: tuple[int, int, int] | None,
    initial_output: pathlib.Path,
    output: pathlib.Path,
    manifest: pathlib.Path,
    screen: bool = False,
) -> dict[str, Any]:
    if text is None and shortcut is None and click is None and scroll is None:
        raise ValueError("provide --text, --shortcut, --click-client, or --scroll-client")
    if sys.platform != "win32":
        raise RuntimeError("native Unicode capture requires Windows")
    encoded, units = _validate_text(text) if text is not None else (b"", ())
    if click is not None:
        _validate_click(*click)
    if scroll is not None:
        _validate_scroll(*scroll)
    executable = _validate_paths(command, initial_output, output, manifest)
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
    try:
        bounds = capture._wait_for_process_window(process)
        layout, foreground_set, focus_target = _focus_window(bounds.handle)
        # Every input claim here is trusted OS input meant to reach the
        # target window, and Windows consumes the first click on an inactive
        # window for activation instead of delivering it (wheel delivery to
        # inactive windows is likewise a user setting), so recording any
        # input claim requires verified activation.
        if not foreground_set:
            raise RuntimeError(
                "Windows denied foreground activation; "
                "no trusted input claim can be recorded"
            )
        if screen:
            # Screen BitBlt reads whatever covers the window's screen rect,
            # so a covering window corrupts every frame even when the
            # capture succeeds; refuse before the initial frame is taken.
            _guard_surface_visible(bounds.handle)
        window_capturer = (
            capture._capture_screen_window if screen else capture._capture_window
        )
        initial = capture._window_observation(bounds.handle)
        initial_pixels = window_capturer(initial.bounds)
        # A composition host may show its window before the page paints;
        # wait for a settled, non-uniform frame so the recorded state is the
        # rendered surface, within the same bounded settle budget.
        initial_settle_rounds = 0
        stable_rounds = 0
        previous_pixels = initial_pixels
        while initial_settle_rounds < SETTLE_ROUNDS:
            if (
                not _blank_frame(initial_pixels, initial.bounds.width, initial.bounds.height)
                and initial_pixels == previous_pixels
            ):
                stable_rounds += 1
                if stable_rounds >= STABLE_ROUNDS:
                    break
            else:
                stable_rounds = 0
            previous_pixels = initial_pixels
            time.sleep(SETTLE_WAIT_SECONDS)
            _flush_window(bounds.handle)
            initial = capture._window_observation(bounds.handle)
            initial_pixels = window_capturer(initial.bounds)
            initial_settle_rounds += 1
        # The hosted content child (a browser render surface) is created
        # asynchronously after the top-level window appears; when keyboard
        # input follows and a content child now exists, move the keyboard
        # focus there so typed text and navigation keys reach the page
        # instead of the host's own focus chain.
        if (text is not None or shortcut is not None) and _content_child(
            bounds.handle
        ) not in (None, focus_target):
            layout, foreground_set, focus_target = _focus_window(bounds.handle)
            if not foreground_set:
                raise RuntimeError(
                    "Windows denied foreground activation; "
                    "no trusted input claim can be recorded"
                )
        keyboard_focus_record = {
            "kind": (
                "content-child" if focus_target != bounds.handle else "top-level"
            ),
            "class": _window_class(focus_target),
        }
        initial_digest = capture._write_capture(initial_output, initial.bounds, initial_pixels)
        scroll_record = None
        if scroll is not None:
            before_scroll = window_capturer(
                capture._window_observation(bounds.handle).bounds
            )
            _require_foreground(bounds.handle)
            _send_wheel(bounds.handle, scroll[0], scroll[1], scroll[2])
            # Let the browser's scroll animation finish so a following click
            # targets the moved content instead of what briefly lies under
            # the cursor.
            scroll_settle_rounds = 0
            stable_rounds = 0
            settled_pixels = window_capturer(
                capture._window_observation(bounds.handle).bounds
            )
            while scroll_settle_rounds < SETTLE_ROUNDS:
                time.sleep(SETTLE_WAIT_SECONDS)
                _flush_window(bounds.handle)
                current_pixels = window_capturer(
                    capture._window_observation(bounds.handle).bounds
                )
                scroll_settle_rounds += 1
                if current_pixels == settled_pixels:
                    stable_rounds += 1
                    if stable_rounds >= STABLE_ROUNDS:
                        break
                else:
                    stable_rounds = 0
                settled_pixels = current_pixels
            scroll_record = {
                "mechanism": "Win32 SendInput MOUSEEVENTF_WHEEL",
                "client_point": [scroll[0], scroll[1]],
                "notches": scroll[2],
                "settle_rounds": scroll_settle_rounds,
                "frame_changed": settled_pixels != before_scroll,
            }
        click_record = None
        if click is not None:
            _require_foreground(bounds.handle)
            screen_x, screen_y = _send_pointer_click(bounds.handle, click[0], click[1])
            click_record = {
                "mechanism": "Win32 SendInput MOUSEEVENTF_ABSOLUTE",
                "client_point": [click[0], click[1]],
                "screen_point": [screen_x, screen_y],
            }
        # Typed text and submit shortcuts follow the pointer phases so they
        # reach the control the click focused.
        _require_foreground(bounds.handle)
        if text is not None:
            _send_unicode_text(units)
        if shortcut is not None:
            _send_shortcut(shortcut)
        _flush_window(bounds.handle)
        after = capture._window_observation(bounds.handle)
        after_pixels = window_capturer(after.bounds)
        # The input-driven update may render after the input queue drains
        # (a click submits through an asynchronous bridge), and a focused
        # text caret keeps alternating frames while the page is idle, so a
        # frame that merely holds still is not evidence of a response.
        settle_rounds = 0
        settle_started = time.monotonic()
        reference_digest = hashlib.sha256(after_pixels).digest()
        recent_digests: list[bytes] = [reference_digest]
        while settle_rounds < SETTLE_ROUNDS:
            time.sleep(SETTLE_WAIT_SECONDS)
            _flush_window(bounds.handle)
            after = capture._window_observation(bounds.handle)
            after_pixels = window_capturer(after.bounds)
            settle_rounds += 1
            frame_digest = hashlib.sha256(after_pixels).digest()
            repeated = frame_digest in recent_digests
            recent_digests.append(frame_digest)
            del recent_digests[:-SETTLE_HISTORY]
            if _after_settled(
                changed=frame_digest != reference_digest,
                repeated=repeated,
                observed_seconds=time.monotonic() - settle_started,
                minimum_seconds=POST_INPUT_MIN_OBSERVE_SECONDS,
            ):
                break
        after_digest = capture._write_capture(output, after.bounds, after_pixels)
        capture._close_window(bounds.handle)
        return_code = process.wait(timeout=capture.PROCESS_EXIT_TIMEOUT_SECONDS)
        if return_code != 0:
            raise RuntimeError(f"native process exited with status {return_code}")
        mechanisms = []
        if text is not None:
            mechanisms.append("Win32 SendInput KEYEVENTF_UNICODE")
        if shortcut is not None:
            mechanisms.append("Win32 SendInput virtual key")
        if click is not None:
            mechanisms.append("Win32 SendInput MOUSEEVENTF_ABSOLUTE")
        result: dict[str, Any] = {
            "process_returncode": return_code,
            "input": {
                "mechanism": " + ".join(mechanisms),
                "utf8_bytes": len(encoded),
                "utf16_units": len(units),
                "text_sha256": (
                    hashlib.sha256(encoded).hexdigest() if text is not None else None
                ),
                "target_keyboard_layout": f"0x{layout:016x}",
                "keyboard_focus": keyboard_focus_record,
                "foreground_window_set": foreground_set,
                "shortcut": shortcut,
                "scroll": scroll_record,
                "click": click_record,
            },
            "capture": {
                "mechanism": "screen BitBlt" if screen else "PrintWindow WM_PRINT",
                "initial_settle_rounds": initial_settle_rounds,
                "after_settle_rounds": settle_rounds,
                "after_observed_seconds": round(time.monotonic() - settle_started, 3),
            },
            "initial": _record(initial, initial_output, initial_digest),
            "after": _record(after, output, after_digest),
            "pixels_changed": initial_pixels != after_pixels,
            "ime": {
                "status": "not_exercised",
                "reason": (
                    "KEYEVENTF_UNICODE delivers committed text; an installed IME "
                    "requires a separate host journey"
                ),
            },
        }
        manifest.parent.mkdir(parents=True, exist_ok=True)
        manifest.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        return result
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=capture.PROCESS_EXIT_TIMEOUT_SECONDS)


def main() -> None:
    if sys.platform != "win32":
        raise SystemExit("python_native_input_capture.py requires a Windows desktop")
    arguments = _parser().parse_args()
    try:
        result = _capture_input(
            arguments.command,
            arguments.command_arguments,
            arguments.cwd,
            arguments.text,
            arguments.shortcut,
            tuple(arguments.click_client) if arguments.click_client is not None else None,
            tuple(arguments.scroll_client) if arguments.scroll_client is not None else None,
            arguments.initial_output.resolve(),
            arguments.output.resolve(),
            arguments.manifest.resolve(),
            screen=arguments.screen,
        )
    except (OSError, RuntimeError, ValueError) as error:
        raise SystemExit(str(error)) from error
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
