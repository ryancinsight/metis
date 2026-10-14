"""Win32 content selection, foreground activation and input-queue focus."""
from __future__ import annotations
import ctypes
import time
from native_input.windows import abi, focus_observation, keyboard


FOCUS_ROUNDS = 20


FOCUS_WAIT_SECONDS = 0.1


def window_class(handle: int) -> str:
    """The Win32 class name of `handle`, empty when it is not a window."""
    user32 = ctypes.windll.user32
    user32.GetClassNameW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]
    user32.GetClassNameW.restype = ctypes.c_int
    name = ctypes.create_unicode_buffer(64)
    user32.GetClassNameW(handle, name, 64)
    return name.value


def pick_content_child(
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


def content_child(handle: int) -> int | None:
    """The handle's hosted content child (see [`pick_content_child`])."""
    user32 = ctypes.windll.user32
    user32.EnumChildWindows.argtypes = [
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.c_void_p,
    ]
    user32.EnumChildWindows.restype = ctypes.c_int32
    user32.IsWindowVisible.argtypes = [ctypes.c_void_p]
    user32.IsWindowVisible.restype = ctypes.c_int32
    user32.GetClientRect.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(abi.Rect),
    ]
    user32.GetClientRect.restype = ctypes.c_int32
    candidates: list[tuple[int, int]] = []

    @ctypes.WINFUNCTYPE(ctypes.c_int32, ctypes.c_void_p, ctypes.c_void_p)
    def collect(child, _parameter):
        if not user32.IsWindowVisible(child):
            return True
        rect = abi.Rect()
        if user32.GetClientRect(child, ctypes.byref(rect)):
            candidates.append(
                (
                    child,
                    max(0, rect.right - rect.left) * max(0, rect.bottom - rect.top),
                )
            )
        return True

    user32.EnumChildWindows(handle, collect, None)
    client = abi.Rect()
    if not user32.GetClientRect(handle, ctypes.byref(client)):
        return None
    client_area = max(0, client.right - client.left) * max(0, client.bottom - client.top)
    return pick_content_child(tuple(candidates), client_area)


def _focus_api():
    user32 = ctypes.windll.user32
    kernel32 = ctypes.windll.kernel32
    user32.BringWindowToTop.argtypes = [ctypes.c_void_p]
    user32.BringWindowToTop.restype = ctypes.c_int32
    user32.GetWindowThreadProcessId.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(ctypes.c_uint32),
    ]
    user32.GetWindowThreadProcessId.restype = ctypes.c_uint32
    user32.AttachThreadInput.argtypes = [ctypes.c_uint32, ctypes.c_uint32, ctypes.c_int32]
    user32.AttachThreadInput.restype = ctypes.c_int32
    user32.SetForegroundWindow.argtypes = [ctypes.c_void_p]
    user32.SetForegroundWindow.restype = ctypes.c_int32
    user32.SetFocus.argtypes = [ctypes.c_void_p]
    user32.SetFocus.restype = ctypes.c_void_p
    user32.GetFocus.argtypes = []
    user32.GetFocus.restype = ctypes.c_void_p
    user32.IsChild.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    user32.IsChild.restype = ctypes.c_int32
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
    user32.SetWindowPos.restype = ctypes.c_int32
    kernel32.GetCurrentThreadId.restype = ctypes.c_uint32
    user32.GetKeyboardLayout.argtypes = [ctypes.c_uint32]
    user32.GetKeyboardLayout.restype = ctypes.c_void_p

    return user32, kernel32


def _activation_matches(user32, handle: int, focus_target: int, ledger: dict) -> bool:
    """Observe native ownership once, before the caller detaches input queues.

    The selected WebView container can delegate focus to a descendant in a
    runtime process. IsChild follows that native parent chain; PID, class and
    thread equality do not express keyboard ownership. A target is not its
    own child, so equality is a separate accepted case. Zero is never focus.
    https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-ischild
    """
    actual_foreground = int(user32.GetForegroundWindow() or 0)
    actual_focus = int(user32.GetFocus() or 0)
    within_target = bool(
        focus_target and actual_focus
        and (actual_focus == focus_target or user32.IsChild(focus_target, actual_focus))
    )
    ledger["criterion"] = {
        "foreground_hwnd": actual_foreground, "focus_hwnd": actual_focus,
        "focus_within_target": within_target,
    }
    return actual_foreground == handle and within_target


def _activate(user32, handle: int, focus_target: int, ledger: dict) -> bool:
    # Trusted input is delivered by hit test and foreground, and a
    # topmost system overlay (for example the shell's screen dimmer)
    # would keep both while a screen capture still shows the target
    # through it; pin the target above such overlays for the recording.
    user32.SetWindowPos(
        handle,
        abi.HWND_TOPMOST,
        0,
        0,
        0,
        0,
        abi.SWP_NOMOVE | abi.SWP_NOSIZE | abi.SWP_NOACTIVATE,
    )
    user32.BringWindowToTop(handle)
    # Foreground activation can lose the race with the window's own
    # creation, and a denied activation makes Windows consume the next
    # click on the window instead of delivering it. A bare ALT tap
    # through the trusted input queue grants the activation right
    # without changing any application state.
    foreground_set = bool(user32.SetForegroundWindow(handle))
    previous = user32.SetFocus(focus_target)
    focus_observation.record_attempt(ledger, previous)
    if not foreground_set:
        keyboard.send_virtual_key(abi.VK_MENU, key_up=False)
        keyboard.send_virtual_key(abi.VK_MENU, key_up=True)
        foreground_set = bool(user32.SetForegroundWindow(handle))
        previous = user32.SetFocus(focus_target)
        focus_observation.record_attempt(ledger, previous)
    focus_rounds = 0
    while (
        focus_rounds < FOCUS_ROUNDS
        and not _activation_matches(user32, handle, focus_target, ledger)
    ):
        focus_rounds += 1
        time.sleep(FOCUS_WAIT_SECONDS)
        # The activation right is consumed by whichever window moves
        # first, so one ALT tap can lose the race; repeat the trusted
        # tap with each bounded retry.
        keyboard.send_virtual_key(abi.VK_MENU, key_up=False)
        keyboard.send_virtual_key(abi.VK_MENU, key_up=True)
        foreground_set = bool(user32.SetForegroundWindow(handle))
        previous = user32.SetFocus(focus_target)
        focus_observation.record_attempt(ledger, previous)
    # All callers receive the same admission contract, including callers that
    # use focus_window only for its verified side effect and ignore its tuple.
    if not _activation_matches(user32, handle, focus_target, ledger):
        foreground_set = ledger["criterion"]["foreground_hwnd"] == handle
        raise RuntimeError(
            f"the window {hex(focus_target)} could not own keyboard focus "
            f"(foreground_set={foreground_set})"
        )
    return ledger["criterion"]["foreground_hwnd"] == handle


def focus_window(handle: int) -> tuple[int, bool, int]:
    """Give `handle`'s keyboard target focus and verify the activation.

    Returns the keyboard layout of the target thread, verified foreground
    activation, and the selected keyboard container. A hosted content child
    (a browser render surface) owns the keyboard and may delegate focus to
    its native descendant, including a WebView runtime process. The returned
    container remains the identity used for asynchronous content selection;
    the actual recipient remains a separate criterion observation.

    # Errors
    Raises `RuntimeError` when the actual foreground differs from `handle`,
    focus belongs to neither the container nor its descendants, or a window
    state read fails.
    """
    user32, kernel32 = _focus_api()
    ledger = {"attempts": [], "criterion": None}
    focus_target = None
    try:
        # A hosted content child (a browser render surface) owns the keyboard;
        # keyboard input must target it instead of the top-level host window.
        focus_target = content_child(handle) or handle
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
    except BaseException as error:
        focus_observation.observe_failure(error, handle, focus_target, ledger)
        raise
    detach_failed = False
    try:
        foreground_set = _activate(user32, handle, focus_target, ledger)
    except BaseException as error:
        focus_observation.observe_failure(error, handle, focus_target, ledger)
        raise
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


def require_foreground(handle: int) -> None:
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
    _, foreground_set, _ = focus_window(handle)
    if not foreground_set:
        raise RuntimeError(
            "Windows denied foreground activation at input delivery; "
            "no trusted input claim can be recorded"
        )
