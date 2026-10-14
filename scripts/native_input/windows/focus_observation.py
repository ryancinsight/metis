"""Read-only, non-atomic focus evidence collected before input queues detach.

No read admits focus, changes a window, pumps messages, retries or sleeps.
The existing activation owner bounds SetFocus attempts by 20 retries plus its
two initial calls. Four window roles and five GUI-thread roles bound reads;
WNDCLASSEX permits 256 class-name characters, so 257 units include termination.
Missing/failed reads remain None with actual operation/native error context.

SetFocus returns the previous HWND; a null return is not a success oracle.
Its immediate native GetLastError sample may be stale and is reported as such.
The original windll activation binding and last-error state remain unchanged.
https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setfocus
https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getfocus
https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getguithreadinfo
https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-guithreadinfo
https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-wndclassexw
https://docs.python.org/3/library/ctypes.html#ctypes.GetLastError
"""

from __future__ import annotations
import ctypes
import json

from native_input.windows import abi


class GuiThreadInfo(ctypes.Structure):
    """DWORD×2 + HWND×6 + RECT: 72 bytes on Win64; 48 bytes on Win32."""
    _fields_ = [
        ("byte_size", ctypes.c_uint32), ("flags", ctypes.c_uint32),
        ("active", ctypes.c_void_p), ("focus", ctypes.c_void_p),
        ("capture", ctypes.c_void_p), ("menu_owner", ctypes.c_void_p),
        ("move_size", ctypes.c_void_p), ("caret", ctypes.c_void_p),
        ("caret_rect", abi.Rect),
    ]


def error_value(operation: str, error: BaseException) -> dict:
    return {"operation": operation, "type": type(error).__name__,
            "message": str(error), "errno": getattr(error, "errno", None),
            "winerror": getattr(error, "winerror", None)}


def attach_report(error: BaseException, report: dict) -> None:
    """Keep the original exception; ASCII JSON preserves Unicode and surrogates."""
    error.focus_observation = report
    error.add_note("win32_focus_observation=" + json.dumps(report, ensure_ascii=True, separators=(",", ":")))


def record_attempt(ledger: dict, previous) -> None:
    """First sample real thread LastError; no native call intervenes after SetFocus."""
    try:
        native_error = ctypes.GetLastError()
    except BaseException as error:
        observed = {"previous_hwnd": int(previous or 0),
                    "native_last_error_after_call": None,
                    "observation_error": error_value("read SetFocus native LastError", error)}
    else:
        observed = {"previous_hwnd": int(previous or 0),
                    "native_last_error_after_call": native_error}
    ledger["attempts"].append(observed)


def _api():
    user = ctypes.WinDLL("user32", use_last_error=True)
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    for name in ("GetFocus", "GetForegroundWindow"):
        function = getattr(user, name)
        function.argtypes, function.restype = [], ctypes.c_void_p
    user.GetWindowThreadProcessId.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32)]
    user.GetWindowThreadProcessId.restype = ctypes.c_uint32
    user.GetClassNameW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]
    user.GetClassNameW.restype = ctypes.c_int
    for name in ("IsWindowVisible", "IsWindowEnabled"):
        function = getattr(user, name)
        function.argtypes, function.restype = [ctypes.c_void_p], ctypes.c_int32
    user.IsChild.argtypes, user.IsChild.restype = [ctypes.c_void_p, ctypes.c_void_p], ctypes.c_int32
    user.GetGUIThreadInfo.argtypes = [ctypes.c_uint32, ctypes.POINTER(GuiThreadInfo)]
    user.GetGUIThreadInfo.restype = ctypes.c_int32
    kernel.GetCurrentThreadId.argtypes, kernel.GetCurrentThreadId.restype = [], ctypes.c_uint32
    return user, kernel


def _read(errors: list, operation: str, function, *arguments):
    try:
        return function(*arguments)
    except BaseException as error:
        errors.append(error_value(operation, error))
        return None


def _native_failure(errors: list, operation: str) -> None:
    code = ctypes.get_last_error()
    errors.append({"operation": operation, "native_last_error": code})


def _window(user, handle, errors: list, role: str) -> dict:
    result = {"hwnd": handle, "thread_id": None, "process_id": None,
              "class_name": None, "visible": None, "enabled": None}
    if not handle:
        return result
    process = ctypes.c_uint32()
    thread = _read(errors, role + ".GetWindowThreadProcessId",
                   user.GetWindowThreadProcessId, handle, ctypes.byref(process))
    if thread:
        result.update({"thread_id": thread, "process_id": process.value})
    elif thread == 0:
        _native_failure(errors, role + ".GetWindowThreadProcessId")
    name = ctypes.create_unicode_buffer(257)
    length = _read(errors, role + ".GetClassNameW", user.GetClassNameW, handle, name, len(name))
    if length:
        result["class_name"] = name.value
    elif length == 0:
        _native_failure(errors, role + ".GetClassNameW")
    for key, method in (("visible", user.IsWindowVisible), ("enabled", user.IsWindowEnabled)):
        value = _read(errors, role + "." + key, method, handle)
        result[key] = bool(value) if value is not None else None
    return result


def _gui_thread(user, thread, errors: list, role: str) -> dict:
    result = {"thread_id": thread, "value": None}
    if not thread:
        return result
    info = GuiThreadInfo(byte_size=ctypes.sizeof(GuiThreadInfo))
    success = _read(errors, role + ".GetGUIThreadInfo", user.GetGUIThreadInfo, thread, ctypes.byref(info))
    if success:
        result["value"] = {name: int(getattr(info, name) or 0)
                           for name in ("active", "focus", "capture", "menu_owner", "move_size", "caret")}
        result["value"].update({"flags": info.flags,
                                "caret_rect": [info.caret_rect.left, info.caret_rect.top,
                                               info.caret_rect.right, info.caret_rect.bottom]})
    elif success == 0:
        _native_failure(errors, role + ".GetGUIThreadInfo")
    return result


def _snapshot(report: dict, top, target) -> None:
    user, kernel = _api()
    errors = report["read_errors"]
    current = _read(errors, "GetCurrentThreadId", kernel.GetCurrentThreadId)
    report["current_thread_id"] = current
    focused = _read(errors, "GetFocus", lambda: int(user.GetFocus() or 0))
    foreground = _read(errors, "GetForegroundWindow", lambda: int(user.GetForegroundWindow() or 0))
    windows = {role: _window(user, handle, errors, role) for role, handle in (
        ("top", top), ("target", target), ("current_focus", focused), ("foreground", foreground))}
    report["windows"] = windows
    for role, parent in (("target", target), ("top", top)):
        child = (_read(errors, f"IsChild({role},current_focus)", user.IsChild, parent, focused)
                 if parent and focused else None)
        report[f"is_child_{role}_current_focus"] = bool(child) if child is not None else None
    report["gui_threads"] = {role: _gui_thread(user, thread, errors, role)
                             for role, thread in (("current", current),
                                 *((key, row["thread_id"]) for key, row in windows.items()))}


def observe_failure(error: BaseException, top, target, ledger: dict) -> None:
    """Attach actual observations; telemetry failures never replace the primary."""
    report = {"schema": 1, "timing": "before_input_queue_detach; non_atomic",
              "activation": ledger, "read_errors": [],
              "last_error_semantics": "raw native observation; may be stale; not a SetFocus success/error oracle"}
    try:
        try:
            _snapshot(report, top, target)
        except BaseException as observation_error:
            report["observation_error"] = error_value("read native focus state", observation_error)
        attach_report(error, report)
    except BaseException as observation_error:
        error.add_note(f"native focus observation failed: {type(observation_error).__name__}: {observation_error}")
