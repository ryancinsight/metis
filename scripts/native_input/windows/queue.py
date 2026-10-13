"""Bounded synchronous flush of a Win32 window message queue."""
from __future__ import annotations
import ctypes
from native_input.windows import abi


INPUT_FLUSH_TIMEOUT_MILLISECONDS = 1_000


def flush_window(handle: int) -> None:
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
    user32.SendMessageTimeoutW.restype = ctypes.c_ssize_t
    result = ctypes.c_size_t()
    if not user32.SendMessageTimeoutW(
        handle,
        abi.WM_NULL,
        0,
        0,
        abi.SMTO_ABORT_IF_HUNG | abi.SMTO_BLOCK,
        INPUT_FLUSH_TIMEOUT_MILLISECONDS,
        ctypes.byref(result),
    ):
        raise ctypes.WinError()
