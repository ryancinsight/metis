"""Run one real actor operation while its HWND owner dispatches messages."""

from concurrent.futures import ThreadPoolExecutor
import ctypes

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
    wait = ctypes.WinDLL("user32", use_last_error=True).MsgWaitForMultipleObjectsEx
    wait.argtypes = [ctypes.c_uint32, ctypes.c_void_p, ctypes.c_uint32,
                     ctypes.c_uint32, ctypes.c_uint32]
    wait.restype = ctypes.c_uint32
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
                # A sent UIA request wakes its HWND owner immediately. Worker
                # completion without input retains the existing maximum poll.
                milliseconds = min(UIA_POLL_MILLISECONDS, int(actor.remaining() * 1000))
                wake = wait(0, None, milliseconds, 0x04FF, 0x0004)
                if wake == 0xFFFFFFFF:
                    raise ctypes.WinError(ctypes.get_last_error())
                if wake not in (0, 258):
                    raise RuntimeError(f"actual fixture message wait returns unexpected status: {wake}")
        return result.result()
