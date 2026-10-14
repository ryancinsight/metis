"""Own actual Windows kill-on-close Job Object admission and retirement."""

from __future__ import annotations

import subprocess


def create(process: subprocess.Popen[bytes]) -> int:
    """Assign a held bootstrap to a kill-on-close Windows Job Object.

    This follows the established Atlas runner in
    ``kwavers/scripts/integration_tests.py``. The bootstrap does not create the
    requested command until assignment succeeds, closing the descendant race
    between process creation and Job Object ownership.
    """
    import ctypes
    from ctypes import wintypes

    class LargeInteger(ctypes.Structure):
        _fields_ = [("quad_part", ctypes.c_longlong)]

    class BasicLimitInformation(ctypes.Structure):
        _fields_ = [
            ("per_process_user_time_limit", LargeInteger),
            ("per_job_user_time_limit", LargeInteger),
            ("limit_flags", wintypes.DWORD),
            ("minimum_working_set_size", ctypes.c_size_t),
            ("maximum_working_set_size", ctypes.c_size_t),
            ("active_process_limit", wintypes.DWORD),
            ("affinity", ctypes.c_size_t),
            ("priority_class", wintypes.DWORD),
            ("scheduling_class", wintypes.DWORD),
        ]

    class IoCounters(ctypes.Structure):
        _fields_ = [
            ("read_operation_count", ctypes.c_ulonglong),
            ("write_operation_count", ctypes.c_ulonglong),
            ("other_operation_count", ctypes.c_ulonglong),
            ("read_transfer_count", ctypes.c_ulonglong),
            ("write_transfer_count", ctypes.c_ulonglong),
            ("other_transfer_count", ctypes.c_ulonglong),
        ]

    class ExtendedLimitInformation(ctypes.Structure):
        _fields_ = [
            ("basic_limit_information", BasicLimitInformation),
            ("io_info", IoCounters),
            ("process_memory_limit", ctypes.c_size_t),
            ("job_memory_limit", ctypes.c_size_t),
            ("peak_process_memory_used", ctypes.c_size_t),
            ("peak_job_memory_used", ctypes.c_size_t),
        ]

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.CreateJobObjectW.argtypes = [ctypes.c_void_p, wintypes.LPCWSTR]
    kernel32.CreateJobObjectW.restype = wintypes.HANDLE
    kernel32.SetInformationJobObject.argtypes = [
        wintypes.HANDLE,
        ctypes.c_int,
        ctypes.c_void_p,
        wintypes.DWORD,
    ]
    kernel32.SetInformationJobObject.restype = wintypes.BOOL
    kernel32.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
    kernel32.AssignProcessToJobObject.restype = wintypes.BOOL
    kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel32.CloseHandle.restype = wintypes.BOOL

    job = kernel32.CreateJobObjectW(None, None)
    if not job:
        raise ctypes.WinError(ctypes.get_last_error())
    information = ExtendedLimitInformation()
    information.basic_limit_information.limit_flags = 0x00002000
    if not kernel32.SetInformationJobObject(
        job, 9, ctypes.byref(information), ctypes.sizeof(information)
    ):
        error = ctypes.WinError(ctypes.get_last_error())
        if kernel32.CloseHandle(job):
            raise error
        close_error = ctypes.WinError(ctypes.get_last_error())
        raise OSError(f"{error}; job handle close failed: {close_error}") from error

    process_handle = getattr(process, "_handle", None)
    if process_handle is None or not kernel32.AssignProcessToJobObject(
        job, wintypes.HANDLE(int(process_handle))
    ):
        error = ctypes.WinError(ctypes.get_last_error())
        if kernel32.CloseHandle(job):
            raise error
        close_error = ctypes.WinError(ctypes.get_last_error())
        raise OSError(f"{error}; job handle close failed: {close_error}") from error
    return int(job)



def close(job: int) -> str | None:
    import ctypes
    from ctypes import wintypes

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel32.CloseHandle.restype = wintypes.BOOL
    if kernel32.CloseHandle(wintypes.HANDLE(job)):
        return None
    return f"job handle close failed: {ctypes.WinError(ctypes.get_last_error())}"



def terminate(job: int, timeout_seconds: float) -> str | None:
    import ctypes
    from ctypes import wintypes

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.TerminateJobObject.argtypes = [wintypes.HANDLE, wintypes.UINT]
    kernel32.TerminateJobObject.restype = wintypes.BOOL
    kernel32.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
    kernel32.WaitForSingleObject.restype = wintypes.DWORD
    if not kernel32.TerminateJobObject(wintypes.HANDLE(job), 1):
        return f"job termination failed: {ctypes.WinError(ctypes.get_last_error())}"
    milliseconds = max(0, min(round(timeout_seconds * 1000), 0xFFFFFFFE))
    wait_status = kernel32.WaitForSingleObject(wintypes.HANDLE(job), milliseconds)
    if wait_status == 0:
        return None
    if wait_status == 0x00000102:
        return f"process tree did not exit within {timeout_seconds:g} seconds"
    if wait_status == 0xFFFFFFFF:
        return f"job wait failed: {ctypes.WinError(ctypes.get_last_error())}"
    return f"job wait returned unexpected status {wait_status}"
