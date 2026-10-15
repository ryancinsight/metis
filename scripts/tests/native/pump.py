"""Run one real actor operation while its HWND owner dispatches messages."""

from concurrent.futures import ThreadPoolExecutor
import ctypes
import time

from native_input.windows import abi
from python_native_accessibility import UIA_POLL_MILLISECONDS


def run_with_pump(user, actor, operation, *arguments):
    user.PeekMessageW.argtypes = [ctypes.POINTER(abi.WindowMessage), ctypes.c_void_p,
                                  ctypes.c_uint32, ctypes.c_uint32, ctypes.c_uint32]
    user.PeekMessageW.restype = ctypes.c_int32
    user.TranslateMessage.argtypes = [ctypes.POINTER(abi.WindowMessage)]
    user.TranslateMessage.restype = ctypes.c_int32
    user.DispatchMessageW.argtypes = [ctypes.POINTER(abi.WindowMessage)]
    user.DispatchMessageW.restype = ctypes.c_ssize_t
    message = abi.WindowMessage()
    with ThreadPoolExecutor(max_workers=1) as executor:
        result = executor.submit(operation, *arguments)
        while not result.done():
            actor.remaining()
            # PM_REMOVE also services real cross-thread WM_GETOBJECT requests.
            # The actor operation retains its original absolute deadline.
            while user.PeekMessageW(ctypes.byref(message), None, 0, 0, 1):
                actor.remaining()
                user.TranslateMessage(ctypes.byref(message))
                user.DispatchMessageW(ctypes.byref(message))
            if not result.done():
                time.sleep(min(actor.remaining(), UIA_POLL_MILLISECONDS / 1000))
        return result.result()
