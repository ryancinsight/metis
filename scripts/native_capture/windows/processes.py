"""Bounded live-descendant enumeration and native window readiness."""
from __future__ import annotations
import ctypes
import subprocess
import sys
import time
from native_capture import limits, model
from native_capture.windows import window
from native_capture.windows import records


def process_tree(root_process_id: int) -> set[int]:
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
        ctypes.POINTER(records.ProcessEntry32W),
    ]
    kernel32.Process32FirstW.restype = ctypes.c_int32
    kernel32.Process32NextW.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(records.ProcessEntry32W),
    ]
    kernel32.Process32NextW.restype = ctypes.c_int32
    kernel32.CloseHandle.argtypes = [ctypes.c_void_p]
    kernel32.CloseHandle.restype = ctypes.c_int32
    children: dict[int, list[int]] = {}
    entry = records.ProcessEntry32W(size=ctypes.sizeof(records.ProcessEntry32W))
    try:
        first = kernel32.Process32FirstW(snapshot, ctypes.byref(entry))
        if not first:
            raise ctypes.WinError()
        while True:
            children.setdefault(entry.parent_process_id, []).append(entry.process_id)
            entry.size = ctypes.sizeof(records.ProcessEntry32W)
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


def wait_for_process_window(process: subprocess.Popen[bytes]) -> model.WindowBounds:
    """Wait for one visible process window using bounded Win32 readiness."""
    if sys.platform != "win32":
        raise RuntimeError("visible native capture requires Windows")

    user32 = ctypes.windll.user32
    kernel32 = ctypes.windll.kernel32
    process_query_limited_information = 0x1000
    synchronize = 0x00100000
    user32.WaitForInputIdle.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
    user32.WaitForInputIdle.restype = ctypes.c_uint32
    kernel32.OpenProcess.argtypes = [ctypes.c_uint32, ctypes.c_int32, ctypes.c_uint32]
    kernel32.OpenProcess.restype = ctypes.c_void_p
    kernel32.CloseHandle.argtypes = [ctypes.c_void_p]
    kernel32.CloseHandle.restype = ctypes.c_int32

    deadline = time.monotonic() + limits.PROCESS_WINDOW_TIMEOUT_SECONDS
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
            return window.window_for_processes(process_tree(process.pid))
        except RuntimeError:
            if process.poll() is not None:
                raise RuntimeError("native process exited before creating a visible window")
            time.sleep(limits.EVENT_WAIT_MILLISECONDS / 1000)
    raise TimeoutError("native process window was not discoverable before the capture deadline")
