"""Exercise bounded subprocess-tree ownership with real descendants."""

from __future__ import annotations

import json
import os
import pathlib
import subprocess
import sys
import tempfile
import textwrap
import unittest
from unittest.mock import patch


SCRIPTS = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import process_tree  # noqa: E402
import verify  # noqa: E402


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
            with (
                patch.object(verify, "ROOT", root),
                patch.object(verify, "OUTPUT", output),
                patch.object(verify, "EVIDENCE", evidence),
            ):
                with self.assertRaisesRegex(SystemExit, "exceeded 1-second budget"):
                    verify.run(
                        "descendant-timeout",
                        [sys.executable, "-c", PARENT, str(lock), CHILD],
                        cwd=root,
                        environment=os.environ.copy(),
                        seconds=1,
                    )

            log = (output / "descendant-timeout.log").read_text(encoding="utf-8")
            self.assertIn("child-ready ", log)
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
