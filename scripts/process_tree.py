"""Run one command with bounded ownership of its complete process tree."""

from __future__ import annotations

import os
import selectors
import signal
import subprocess
import sys
import tempfile
import time
from collections.abc import Mapping, Sequence
from dataclasses import dataclass


PROCESS_TREE_CLEANUP_SECONDS = 10.0
_WINDOWS_PROCESS_BOOTSTRAP = """
import subprocess
import sys

if sys.stdin.buffer.read(1) != b"1":
    raise SystemExit("process-tree bootstrap was not released")
raise SystemExit(subprocess.run(sys.argv[1:], check=False).returncode)
"""
_POSIX_PROCESS_SUPERVISOR = """
import os
import signal
import subprocess
import sys

status = int(sys.argv[1])
try:
    result = subprocess.run(sys.argv[2:], check=False)
except BaseException as error:
    message = f"error:{type(error).__name__}:{error}".encode("utf-8", errors="replace")
    os.write(status, message)
    os.close(status)
    raise
os.write(status, f"exit:{result.returncode}".encode("ascii"))
os.close(status)
while True:
    signal.pause()
"""


class ProcessTreeTimeout(subprocess.TimeoutExpired):
    """A command exceeded its deadline after bounded process-tree cleanup."""

    def __init__(
        self,
        command: Sequence[str],
        timeout: float,
        stdout: str,
        stderr: str,
        cleanup_error: str | None,
    ) -> None:
        super().__init__(command, timeout, output=stdout, stderr=stderr)
        self.cleanup_error = cleanup_error


@dataclass
class _CleanupState:
    """Track destructive cleanup so interruption cannot repeat it after reap."""

    termination_issued: bool = False
    launcher_reaped: bool = False
    deadline: float | None = None


def _merge_errors(*errors: str | None) -> str | None:
    messages = [error for error in errors if error]
    return "; ".join(messages) if messages else None


def _create_windows_kill_job(process: subprocess.Popen[bytes]) -> int:
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


def _close_windows_job(job: int) -> str | None:
    import ctypes
    from ctypes import wintypes

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel32.CloseHandle.restype = wintypes.BOOL
    if kernel32.CloseHandle(wintypes.HANDLE(job)):
        return None
    return f"job handle close failed: {ctypes.WinError(ctypes.get_last_error())}"


def _terminate_windows_job(job: int, timeout_seconds: float) -> str | None:
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


def _terminate_process_tree(
    process: subprocess.Popen[bytes], windows_job: int | None, state: _CleanupState
) -> str | None:
    """Stop the owned tree and bound collection of its launcher."""
    if state.deadline is None:
        state.deadline = time.monotonic() + PROCESS_TREE_CLEANUP_SECONDS
    cleanup_deadline = state.deadline
    error: str | None = None
    if not state.termination_issued:
        # The unreaped supervisor pins its POSIX process-group identity. An
        # interrupted syscall may therefore retry safely; confirmation is
        # recorded only after the destructive request returns.
        if sys.platform == "win32":
            if windows_job is None:
                error = "process was not assigned to a Windows kill-on-close job"
            else:
                remaining = max(0.0, cleanup_deadline - time.monotonic())
                error = _terminate_windows_job(windows_job, remaining)
        else:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            except OSError as caught:
                error = f"process-group kill failed: {caught}"
        state.termination_issued = True

    if error is not None and not state.launcher_reaped:
        process.kill()
    if not state.launcher_reaped:
        remaining = max(0.0, cleanup_deadline - time.monotonic())
        try:
            process.wait(timeout=remaining)
        except subprocess.TimeoutExpired:
            process.kill()
            remaining = max(0.0, cleanup_deadline - time.monotonic())
            try:
                process.wait(timeout=remaining)
            except subprocess.TimeoutExpired:
                direct_error = "launcher did not exit within the process-tree cleanup budget"
                error = _merge_errors(error, direct_error)
        if process.returncode is not None:
            state.launcher_reaped = True
    if sys.platform != "win32" and state.launcher_reaped:
        while True:
            if not _posix_group_has_active_members(process.pid):
                break
            remaining = cleanup_deadline - time.monotonic()
            if remaining <= 0:
                error = _merge_errors(
                    error, "process group remained active after the cleanup budget"
                )
                break
            time.sleep(min(0.01, remaining))
    return error


def _posix_group_has_active_members(process_group: int) -> bool:
    """Report executable group members while treating Linux zombies as inactive.

    Linux exposes each member's state through ``/proc``; ``Z`` and ``X`` have
    already released runtime resources and await only reaping. Other POSIX
    hosts conservatively retain the group until the kernel removes it.
    """
    if sys.platform.startswith("linux") and os.path.isdir("/proc"):
        with os.scandir("/proc") as entries:
            for entry in entries:
                if not entry.name.isdecimal():
                    continue
                try:
                    with open(entry.path + "/stat", encoding="utf-8") as status:
                        stat = status.read()
                except (FileNotFoundError, ProcessLookupError):
                    continue
                except OSError:
                    return True
                fields = stat[stat.rfind(")") + 1 :].split()
                if len(fields) >= 3 and fields[2].isdecimal():
                    active = fields[0] not in {"Z", "X", "x"}
                    if int(fields[2]) == process_group and active:
                        return True
        return False
    try:
        os.killpg(process_group, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def _read_output(stream) -> str:
    stream.seek(0)
    decoded = stream.read().decode("utf-8", errors="replace")
    return decoded.replace("\r\n", "\n").replace("\r", "\n")


def _read_posix_status(status: int, timeout: float) -> int | None:
    """Return the supervised command status, or ``None`` at the deadline."""
    with selectors.DefaultSelector() as selector:
        selector.register(status, selectors.EVENT_READ)
        if not selector.select(timeout):
            return None
    message = os.read(status, 4096).decode("utf-8", errors="replace")
    if message.startswith("exit:"):
        try:
            return int(message.removeprefix("exit:"))
        except ValueError as error:
            raise RuntimeError(f"invalid process supervisor status: {message!r}") from error
    if message.startswith("error:"):
        raise RuntimeError(f"process supervisor could not launch command: {message[6:]}")
    raise RuntimeError(f"process supervisor returned invalid status: {message!r}")


def run(
    command: Sequence[str],
    *,
    cwd: str | os.PathLike[str] | None = None,
    env: Mapping[str, str] | None = None,
    timeout: float,
) -> subprocess.CompletedProcess[str]:
    """Run ``command`` and retire its owned process tree within finite bounds."""
    if timeout <= 0:
        raise ValueError("process timeout must be greater than zero")
    arguments = list(command)
    started = time.monotonic()
    deadline = started + timeout
    windows_job: int | None = None
    process: subprocess.Popen[bytes] | None = None
    command_returncode: int | None = None
    status_read: int | None = None
    status_write: int | None = None
    timed_out = False
    cleanup_error: str | None = None
    caught_error: BaseException | None = None
    cleanup_state = _CleanupState()
    with tempfile.TemporaryFile(mode="w+b") as stdout, tempfile.TemporaryFile(
        mode="w+b"
    ) as stderr:
        if sys.platform == "win32":
            launch_command = [sys.executable, "-c", _WINDOWS_PROCESS_BOOTSTRAP, *arguments]
            process_options = {
                "creationflags": subprocess.CREATE_NEW_PROCESS_GROUP,
                "stdin": subprocess.PIPE,
            }
        else:
            status_read, status_write = os.pipe()
            launch_command = [
                sys.executable,
                "-c",
                _POSIX_PROCESS_SUPERVISOR,
                str(status_write),
                *arguments,
            ]
            process_options = {
                "pass_fds": (status_write,),
                "start_new_session": True,
            }
        try:
            process = subprocess.Popen(
                launch_command,
                cwd=cwd,
                env=env,
                stdout=stdout,
                stderr=stderr,
                **process_options,
            )
            if status_write is not None:
                os.close(status_write)
                status_write = None
            if sys.platform == "win32":
                try:
                    windows_job = _create_windows_kill_job(process)
                except OSError as caught:
                    raise RuntimeError(
                        f"failed to establish process-tree ownership: {caught}"
                    ) from caught
                remaining = deadline - time.monotonic()
                if remaining > 0:
                    if process.stdin is None:
                        raise RuntimeError("Windows process-tree bootstrap has no control pipe")
                    process.stdin.write(b"1")
                    process.stdin.close()
                else:
                    timed_out = True
            if sys.platform != "win32" and not timed_out:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    timed_out = True
                else:
                    command_returncode = _read_posix_status(status_read, remaining)
                    timed_out = command_returncode is None
            elif not timed_out:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    timed_out = True
                else:
                    try:
                        process.wait(timeout=remaining)
                    except subprocess.TimeoutExpired:
                        timed_out = True
                if not timed_out:
                    command_returncode = process.returncode
            cleanup_error = _terminate_process_tree(process, windows_job, cleanup_state)
        except BaseException as caught:
            caught_error = caught
            if process is not None:
                cleanup_error = _terminate_process_tree(
                    process, windows_job, cleanup_state
                )
            if cleanup_error is not None:
                caught.add_note(f"process-tree cleanup failed: {cleanup_error}")
            raise
        finally:
            if process is not None and process.stdin is not None and not process.stdin.closed:
                process.stdin.close()
            if status_write is not None:
                os.close(status_write)
            if status_read is not None:
                os.close(status_read)
            if windows_job is not None:
                close_error = _close_windows_job(windows_job)
                cleanup_error = _merge_errors(cleanup_error, close_error)
                if close_error is not None and caught_error is not None:
                    caught_error.add_note(f"process-tree cleanup failed: {close_error}")

        captured_stdout = _read_output(stdout)
        captured_stderr = _read_output(stderr)
        if timed_out:
            raise ProcessTreeTimeout(
                arguments,
                timeout,
                captured_stdout,
                captured_stderr,
                cleanup_error,
            )
        if cleanup_error is not None:
            raise RuntimeError(f"process-tree cleanup failed: {cleanup_error}")
        return subprocess.CompletedProcess(
            arguments,
            command_returncode,
            captured_stdout,
            captured_stderr,
        )
