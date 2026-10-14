"""Exercise bounded subprocess-tree ownership with real descendants."""

from __future__ import annotations

import json
import errno
import os
import pathlib
import socket
import subprocess
import sys
import tempfile
import textwrap
import threading
import time
import unittest
from unittest.mock import patch


SCRIPTS = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import process_tree  # noqa: E402
import verify  # noqa: E402
from webview_acceptance import dispatch, retention


CHILD = textwrap.dedent(
    """
    import os
    import pathlib
    import sys
    import threading

    lock = pathlib.Path(sys.argv[1]).open("r+b")
    if os.name == "nt":
        import msvcrt
        lock.seek(0)
        msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
    else:
        import fcntl
        fcntl.flock(lock, fcntl.LOCK_EX)
    ready_port = os.environ.get("METIS_TEST_READY_PORT")
    if ready_port:
        import socket
        socket.create_connection(("127.0.0.1", int(ready_port))).close()
    print(f"child-ready {os.getpid()}", flush=True)
    threading.Event().wait()
    """
)

PARENT = textwrap.dedent(
    """
    import subprocess
    import sys

    child = subprocess.Popen(
        [sys.executable, "-c", sys.argv[2], sys.argv[1]],
        stdout=subprocess.PIPE,
        text=True,
    )
    ready = child.stdout.readline()
    if not ready.startswith("child-ready "):
        raise SystemExit(f"invalid child readiness signal: {ready!r}")
    print(ready, end="", flush=True)
    raise SystemExit(child.wait())
    """
)

EXITING_PARENT = textwrap.dedent(
    """
    import subprocess
    import sys

    child = subprocess.Popen(
        [sys.executable, "-c", sys.argv[2], sys.argv[1]],
        stdout=subprocess.PIPE,
        text=True,
    )
    ready = child.stdout.readline()
    if not ready.startswith("child-ready "):
        raise SystemExit(f"invalid child readiness signal: {ready!r}")
    print(ready, end="", flush=True)
    """
)


# Bounds only a child that never starts: the test then fails at this deadline.
# A child that starts signals within its launch latency and never waits here.
READY_SECONDS = 60


def _budget_from_readiness(listener: socket.socket):
    """Hold the supervisor's clock and first wait until the child signals.

    A budget counted from launch can expire before two interpreter launches
    finish on a loaded host, with no descendant yet to retire. Counting from
    readiness keeps the one-second budget and makes it expire with the child
    alive, holding its lock.
    """
    ready = threading.Event()
    real_monotonic = time.monotonic
    frozen: list[float] = []

    def clock() -> float:
        if ready.is_set():
            return real_monotonic()
        if not frozen:
            frozen.append(real_monotonic())
        return frozen[0]

    def after_ready(wait):
        def supervised(*args, **kwargs):
            if not ready.is_set():
                connection, _ = listener.accept()
                connection.close()
                ready.set()
            return wait(*args, **kwargs)

        return supervised

    return (
        patch.object(process_tree.time, "monotonic", clock),
        patch.object(process_tree.subprocess.Popen, "wait", after_ready(subprocess.Popen.wait)),
        patch.object(
            process_tree, "_read_posix_status", after_ready(process_tree._read_posix_status)
        ),
    )


def _assert_lock_released(test: unittest.TestCase, path: pathlib.Path) -> None:
    with path.open("r+b") as lock:
        if os.name == "nt":
            import msvcrt

            lock.seek(0)
            try:
                msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
            except OSError as error:
                test.fail(f"timed-out descendant retained its file lock: {error}")
            lock.seek(0)
            msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
        else:
            import fcntl

            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                test.fail("timed-out descendant retained its file lock")


def _assert_process_inactive(test: unittest.TestCase, process_id: int) -> None:
    if os.name == "nt":
        listing = subprocess.run(
            ["tasklist", "/FI", f"PID eq {process_id}", "/FO", "CSV", "/NH"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=5,
            check=False,
        )
        test.assertNotIn(f'"{process_id}"', listing.stdout)
        return
    status = subprocess.run(
        ["ps", "-o", "stat=", "-p", str(process_id)],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=5,
        check=False,
    )
    test.assertTrue(
        status.returncode != 0 or not status.stdout.strip() or status.stdout.lstrip().startswith("Z"),
        f"descendant {process_id} remains active with state {status.stdout.strip()!r}",
    )


class ProcessTreeTests(unittest.TestCase):
    """Keep ordinary status and diagnostics while containing descendants."""

    def test_normal_exit_preserves_output(self):
        result = process_tree.run(
            [
                sys.executable,
                "-c",
                "import sys; print('standard output'); print('standard error', file=sys.stderr)",
            ],
            timeout=5,
        )

        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "standard output\n")
        self.assertEqual(result.stderr, "standard error\n")

    def test_nonzero_exit_preserves_status_and_output(self):
        result = process_tree.run(
            [sys.executable, "-c", "import sys; print('rejected'); raise SystemExit(7)"],
            timeout=5,
        )

        self.assertEqual(result.returncode, 7)
        self.assertEqual(result.stdout, "rejected\n")
        self.assertEqual(result.stderr, "")

    def test_failure_outcome_keeps_real_child_streams_status_and_native_error_identity(self):
        """Exercise outcome construction, without simulating OS tree cleanup."""
        command = [sys.executable, "-c", "import sys; value=sum(range(257)); "
                   "print(f'computed={value} λ🦀'); print(f'modulus={value%29} Ω',file=sys.stderr); "
                   "raise SystemExit(value%29)"]
        started = time.monotonic()
        actual = process_tree.run(command, timeout=READY_SECONDS,
                                  env={**os.environ, "PYTHONUTF8": "1"})
        elapsed = time.monotonic() - started
        expected = 256 * 257 // 2
        self.assertEqual(actual.returncode, expected % 29)
        self.assertEqual(actual.stdout, f"computed={expected} λ🦀\n")
        self.assertEqual(actual.stderr, f"modulus={expected % 29} Ω\n")
        temporary = tempfile.TemporaryFile()
        os.close(temporary.fileno())
        try:
            temporary.close()
        except OSError as error:
            native = error
        else:
            self.fail("native close unexpectedly admits an already closed descriptor")
        self.assertEqual(native.errno, errno.EBADF)
        with self.assertRaises(type(native)) as preserved:
            process_tree._outcome(command, actual.returncode, actual.stdout, actual.stderr,
                                  None, native)
        self.assertIs(preserved.exception, native)
        self.assertEqual(preserved.exception.stdout, actual.stdout)
        self.assertEqual(preserved.exception.stderr, actual.stderr)
        self.assertEqual(preserved.exception.cmd, command)
        self.assertEqual(preserved.exception.returncode, actual.returncode)
        self.assertIsNone(preserved.exception.cleanup_error)
        details = f"{type(native).__name__}: {native}"
        with self.assertRaisesRegex(RuntimeError, "process-tree cleanup failed") as cleanup:
            process_tree._outcome(command, actual.returncode, actual.stdout, actual.stderr,
                                  details, None)
        self.assertEqual(cleanup.exception.stdout, actual.stdout)
        self.assertEqual(cleanup.exception.stderr, actual.stderr)
        self.assertEqual(cleanup.exception.cmd, command)
        self.assertEqual(cleanup.exception.returncode, expected % 29)
        self.assertEqual(cleanup.exception.cleanup_error, details)
        self.assertIn(str(native), str(cleanup.exception))
        try:
            process_tree._read_output(temporary)
        except ValueError as error:
            unavailable = error
        else:
            self.fail("closed native capture unexpectedly remains readable")
        with self.assertRaises(ValueError) as missing:
            process_tree._outcome(command, actual.returncode, None, actual.stderr,
                                  None, unavailable)
        self.assertIs(missing.exception, unavailable)
        self.assertIsNone(missing.exception.stdout)
        self.assertEqual(missing.exception.stderr, actual.stderr)
        with self.assertRaisesRegex(RuntimeError, "unavailable capture without a failure"):
            process_tree._outcome(command, actual.returncode, None, actual.stderr, None, None)
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory)
            observed = dispatch.Outcome(tuple(command), unavailable.returncode, unavailable.stdout,
                                        unavailable.stderr, elapsed, unavailable)
            retained = retention.processes((observed,), output, ("unavailable",))
            self.assertFalse(retained.records[0]["stdout_available"])
            self.assertTrue(retained.records[0]["stderr_available"])
            self.assertEqual(retained.records[0]["seconds"], elapsed)
            self.assertFalse((output / "unavailable.stdout").exists())
            self.assertFalse((output / "unavailable.log").exists())
            self.assertEqual((output / "unavailable.stderr").read_text(), actual.stderr)

    def test_real_early_launch_error_preserves_native_cause_and_available_streams(self):
        with tempfile.TemporaryDirectory() as directory:
            missing = pathlib.Path(directory) / "missing"
            command = [sys.executable, "-c", "print(sum(range(257)))"]
            try:
                subprocess.run(command, cwd=missing, capture_output=True, timeout=READY_SECONDS, check=True)
            except OSError as error:
                native = error
            else:
                self.fail("native launch unexpectedly admits a nonexistent working directory")
            with self.assertRaises(type(native)) as caught:
                process_tree.run(command, cwd=missing, timeout=READY_SECONDS)
            self.assertEqual(caught.exception.errno, native.errno)
            self.assertEqual(caught.exception.filename, native.filename)
            self.assertEqual(getattr(caught.exception, "winerror", None), getattr(native, "winerror", None))
            self.assertEqual(str(caught.exception), str(native))
            self.assertEqual(caught.exception.cmd, command)
            self.assertIsNone(caught.exception.returncode)
            self.assertEqual(caught.exception.stdout, "")
            self.assertEqual(caught.exception.stderr, "")
            self.assertIsNone(caught.exception.cleanup_error)

    def test_cleanup_never_reissues_posix_group_signal_after_reap(self):
        class ReapedLauncher:
            pid = 2_147_483_000
            returncode = None

            def kill(self):
                self.returncode = -9

            def wait(self, timeout):
                del timeout
                self.returncode = -9
                return self.returncode

        launcher = ReapedLauncher()
        state = process_tree._CleanupState()
        with (
            patch.object(process_tree.sys, "platform", "linux"),
            patch.object(process_tree.os, "killpg", create=True) as kill_group,
            patch.object(process_tree.signal, "SIGKILL", 9, create=True),
            patch.object(process_tree, "_posix_group_has_active_members", return_value=False),
        ):
            first = process_tree._terminate_process_tree(launcher, None, state)
            second = process_tree._terminate_process_tree(launcher, None, state)

        self.assertIsNone(first)
        self.assertIsNone(second)
        self.assertTrue(state.launcher_reaped)
        kill_group.assert_called_once_with(launcher.pid, 9)

    def test_cleanup_retries_interrupted_signal_while_group_identity_is_owned(self):
        class OwnedLauncher:
            pid = 2_147_483_000
            returncode = None

            def kill(self):
                self.returncode = -9

            def wait(self, timeout):
                del timeout
                self.returncode = -9
                return self.returncode

        launcher = OwnedLauncher()
        state = process_tree._CleanupState()
        with (
            patch.object(process_tree.sys, "platform", "linux"),
            patch.object(
                process_tree.os,
                "killpg",
                side_effect=(KeyboardInterrupt(), None),
                create=True,
            ) as kill_group,
            patch.object(process_tree.signal, "SIGKILL", 9, create=True),
            patch.object(process_tree, "_posix_group_has_active_members", return_value=False),
        ):
            with self.assertRaises(KeyboardInterrupt):
                process_tree._terminate_process_tree(launcher, None, state)
            self.assertFalse(state.termination_issued)
            self.assertFalse(state.launcher_reaped)
            cleanup_error = process_tree._terminate_process_tree(launcher, None, state)

        self.assertIsNone(cleanup_error)
        self.assertTrue(state.termination_issued)
        self.assertTrue(state.launcher_reaped)
        self.assertEqual(kill_group.call_count, 2)

    def test_normal_exit_retires_descendant(self):
        with tempfile.TemporaryDirectory(prefix="metis-process-tree-exit-") as directory:
            lock = pathlib.Path(directory) / "descendant.lock"
            lock.write_bytes(b"x")
            result = process_tree.run(
                [sys.executable, "-c", EXITING_PARENT, str(lock), CHILD], timeout=5
            )

            self.assertEqual(result.returncode, 0)
            process_id = int(result.stdout.split("child-ready ", 1)[1].strip())
            _assert_lock_released(self, lock)
            _assert_process_inactive(self, process_id)

    def test_timeout_reports_gate_budget_and_retires_descendant(self):
        with tempfile.TemporaryDirectory(
            prefix="metis-process-tree-test-", dir=verify.PHYSICAL_ROOT / "output"
        ) as directory:
            root = pathlib.Path(directory)
            output = root / "output"
            output.mkdir()
            lock = root / "descendant.lock"
            lock.write_bytes(b"x")
            evidence = {"schema": 1, "status": "running", "stages": {}, "commands": {}}
            listener = socket.create_server(("127.0.0.1", 0))
            listener.settimeout(READY_SECONDS)
            environment = os.environ.copy()
            environment["METIS_TEST_READY_PORT"] = str(listener.getsockname()[1])
            clock, windows_wait, posix_wait = _budget_from_readiness(listener)
            with (
                listener,
                clock,
                windows_wait,
                posix_wait,
                patch.object(verify, "ROOT", root),
                patch.object(verify, "OUTPUT", output),
                patch.object(verify, "EVIDENCE", evidence),
            ):
                with self.assertRaisesRegex(SystemExit, "exceeded 1-second budget") as stopped:
                    verify.run(
                        "descendant-timeout",
                        [sys.executable, "-c", PARENT, str(lock), CHILD],
                        cwd=root,
                        environment=environment,
                        seconds=1,
                    )

            actual_timeout = stopped.exception.__cause__
            self.assertIsInstance(actual_timeout, process_tree.ProcessTreeTimeout)
            self.assertEqual(actual_timeout.timeout, 1)
            self.assertEqual(actual_timeout.cmd, [sys.executable, "-c", PARENT, str(lock), CHILD])
            self.assertIsNone(actual_timeout.returncode)
            self.assertIsNone(actual_timeout.cleanup_error)
            self.assertEqual(actual_timeout.stderr, "")
            log = (output / "descendant-timeout.log").read_text(encoding="utf-8")
            self.assertIn("child-ready ", log)
            self.assertIn(actual_timeout.stdout, log)
            self.assertIn("descendant-timeout: exceeded 1-second budget", log)
            process_id = int(log.split("child-ready ", 1)[1].splitlines()[0])
            _assert_lock_released(self, lock)
            _assert_process_inactive(self, process_id)
            report = json.loads((output / "verification.json").read_text(encoding="utf-8"))
            self.assertEqual(report["stages"]["descendant-timeout"], "running")
            self.assertEqual(
                report["commands"]["descendant-timeout"]["timeout_seconds"], 1
            )


if __name__ == "__main__":
    unittest.main()
