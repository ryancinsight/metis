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

from process_tools import windows_job


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
        stdout: str | None,
        stderr: str | None,
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


@dataclass
class _ProcessState:
    """Track only resources actually acquired by the current command."""

    process: subprocess.Popen[bytes] | None = None
    job: int | None = None
    status_read: int | None = None
    status_write: int | None = None


def _merge_errors(*errors: str | None) -> str | None:
    messages = [error for error in errors if error]
    return "; ".join(messages) if messages else None




def _terminate_process_tree(
    process: subprocess.Popen[bytes], job: int | None, state: _CleanupState
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
            if job is None:
                error = "process was not assigned to a Windows kill-on-close job"
            else:
                remaining = max(0.0, cleanup_deadline - time.monotonic())
                error = windows_job.terminate(job, remaining)
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


def _launch(arguments, cwd, environment, stdout, stderr, state: _ProcessState) -> None:
    if sys.platform == "win32":
        command = [sys.executable, "-c", _WINDOWS_PROCESS_BOOTSTRAP, *arguments]
        options = {"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP, "stdin": subprocess.PIPE}
    else:
        state.status_read, state.status_write = os.pipe()
        command = [sys.executable, "-c", _POSIX_PROCESS_SUPERVISOR, str(state.status_write), *arguments]
        options = {"pass_fds": (state.status_write,), "start_new_session": True}
    state.process = subprocess.Popen(command, cwd=cwd, env=environment, stdout=stdout, stderr=stderr, **options)
    if state.status_write is not None:
        os.close(state.status_write)
        state.status_write = None
    if sys.platform == "win32":
        try:
            state.job = windows_job.create(state.process)
        except OSError as error:
            raise RuntimeError(f"failed to establish process-tree ownership: {error}") from error


def _wait_command(state: _ProcessState, deadline: float) -> tuple[int | None, bool]:
    process = state.process
    if process is None:
        raise RuntimeError("process-tree command has no acquired launcher")
    if sys.platform == "win32":
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return None, True
        if process.stdin is None:
            raise RuntimeError("Windows process-tree bootstrap has no control pipe")
        process.stdin.write(b"1")
        process.stdin.close()
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        return None, True
    if sys.platform != "win32":
        if state.status_read is None:
            raise RuntimeError("process-tree command has no acquired status pipe")
        status = _read_posix_status(state.status_read, remaining)
        return status, status is None
    try:
        process.wait(timeout=remaining)
    except subprocess.TimeoutExpired:
        return None, True
    return process.returncode, False


def _close_controls(state: _ProcessState) -> tuple[list[BaseException], str | None]:
    failures = []
    if state.process is not None and state.process.stdin is not None and not state.process.stdin.closed:
        try:
            state.process.stdin.close()
        except BaseException as error:
            error.add_note("closing actual process-tree bootstrap input")
            failures.append(error)
    for descriptor in (state.status_write, state.status_read):
        if descriptor is not None:
            try:
                os.close(descriptor)
            except BaseException as error:
                error.add_note(f"closing actual process-tree status descriptor {descriptor}")
                failures.append(error)
    close_error = None
    if state.job is not None:
        try:
            close_error = windows_job.close(state.job)
        except BaseException as error:
            error.add_note("closing actual Windows kill-on-close Job Object")
            failures.append(error)
    return failures, close_error


def _outcome(arguments, returncode, stdout, stderr, cleanup_error, primary):
    """Publish available canonical text on the original failure object.

    None denotes unavailable capture after an actual allocation/read failure.
    Empty text denotes a successful read that observes no output. The real child
    status remains None when no status was observed; no launcher status replaces it.
    """
    if primary is None and cleanup_error is not None:
        primary = RuntimeError(f"process-tree cleanup failed: {cleanup_error}")
    if primary is not None:
        primary.cmd = arguments
        primary.returncode = returncode
        primary.stdout = stdout
        primary.stderr = stderr
        primary.cleanup_error = cleanup_error
        if cleanup_error is not None:
            primary.add_note(f"process-tree cleanup failed: {cleanup_error}")
        raise primary
    if stdout is None or stderr is None:
        raise RuntimeError("process-tree outcome has unavailable capture without a failure")
    return subprocess.CompletedProcess(arguments, returncode, stdout, stderr)


def run(
    command: Sequence[str],
    *,
    cwd: str | os.PathLike[str] | None = None,
    env: Mapping[str, str] | None = None,
    timeout: float,
) -> subprocess.CompletedProcess[str]:
    """Run one owned tree; failures retain available text, status and native cause."""
    if timeout <= 0:
        raise ValueError("process timeout must be greater than zero")
    arguments = list(command)
    deadline = time.monotonic() + timeout
    state, cleanup_state = _ProcessState(), _CleanupState()
    primary = None
    failures = []
    cleanup_error = None
    returncode, timed_out = None, False
    captures = []
    texts = [None, None]
    try:
        captures.append(tempfile.TemporaryFile(mode="w+b"))
        captures.append(tempfile.TemporaryFile(mode="w+b"))
        try:
            _launch(arguments, cwd, env, *captures, state)
            returncode, timed_out = _wait_command(state, deadline)
            if timed_out:
                # The observed deadline precedes every collection/capture error.
                primary = ProcessTreeTimeout(arguments, timeout, None, None, None)
            cleanup_error = _terminate_process_tree(state.process, state.job, cleanup_state)
        except BaseException as error:
            if primary is None:
                primary = error
            else:
                failures.append(error)
                cleanup_error = _merge_errors(cleanup_error, str(error))
            if state.process is not None:
                try:
                    cleanup_error = _merge_errors(
                        cleanup_error, _terminate_process_tree(state.process, state.job, cleanup_state))
                except BaseException as cleanup:
                    cleanup.add_note("collecting the actual process tree after its original failure")
                    failures.append(cleanup)
                    cleanup_error = _merge_errors(cleanup_error, str(cleanup))
        finally:
            control_errors, close_error = _close_controls(state)
            failures.extend(control_errors)
            cleanup_error = _merge_errors(cleanup_error, close_error)
        for index, capture in enumerate(captures):
            try:
                texts[index] = _read_output(capture)
            except BaseException as error:
                error.add_note(f"reading actual process-tree captured {('stdout', 'stderr')[index]}")
                failures.append(error)
    except BaseException as error:
        if primary is None:
            primary = error
        else:
            failures.append(error)
    finally:
        for capture in reversed(captures):
            try:
                capture.close()
            except BaseException as error:
                error.add_note("closing actual process-tree capture")
                failures.append(error)
                cleanup_error = _merge_errors(cleanup_error, str(error))
    for error in failures:
        if primary is None:
            primary = error
        else:
            primary.add_note(f"additional actual process-tree failure: {type(error).__name__}: {error}; "
                             f"{'; '.join(getattr(error, '__notes__', []))}")
    return _outcome(arguments, returncode, *texts, cleanup_error, primary)
