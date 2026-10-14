"""Real process/clock evidence and read-only startup ownership contracts."""

import ast
from contextlib import contextmanager
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))

from webview_acceptance import scenario, startup
from webview_acceptance.contract import FIXTURE_SECONDS


CHILD = """
import json, os, pathlib, sys
sys.stdin.read(1)
if sys.argv[2] == 'fail':
    1 // 0
value = sum(range(257))
pathlib.Path(sys.argv[1]).write_text(json.dumps({'fields': {
    'event': 'calculated', 'pid': os.getpid(), 'value': value}}) + '\\n')
sys.stdin.read()
"""


@contextmanager
def child(root: Path, mode: str = "calculate"):
    trace = root / "native.jsonl"
    trace.touch()
    with (root / "child.stderr").open("wb") as stderr:
        process = subprocess.Popen(
            [sys.executable, "-c", CHILD, str(trace), mode],
            stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=stderr,
        )
        try:
            yield process, trace
        finally:
            if process.poll() is None:
                process.kill()
            process.wait(timeout=FIXTURE_SECONDS)
            process.stdin.close()


class StartupObservation(unittest.TestCase):
    def test_real_child_value_remains_separate_from_native_observation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with child(root) as (process, trace):
                deadline = time.monotonic() + FIXTURE_SECONDS
                observation = startup.StartupObservation(process.pid, deadline, root)
                observation.sample()
                process.stdin.write(b"x")
                process.stdin.flush()
                actual = scenario.wait_event(
                    process, trace, "calculated", deadline, observation=observation,
                )
                observation.finish()
                self.assertEqual(actual, {"event": "calculated", "pid": process.pid,
                                          "value": 256 * 257 // 2})
                saved = json.loads((root / "startup.json").read_text())
                self.assertEqual(saved, observation.report)
                self.assertEqual(saved["native_process_id"], process.pid)
                self.assertEqual(saved["deadline_monotonic"], deadline)
                self.assertGreaterEqual(saved["attempts"], 1)
                self.assertNotIn("page_ready", trace.read_text())
                self.assertEqual(saved["changed_frames"], 0)
                self.assertIsNone(saved["latest_capture"])
                self.assertFalse((root / "startup.bmp").exists())
                errors = saved["observer_errors"]
                operation = "platform" if sys.platform != "win32" else "window"
                self.assertEqual(set(errors), {operation})
                observed = errors[operation]
                self.assertEqual(observed["count"], saved["attempts"])
                for record in (observed["first"], observed["latest"]):
                    self.assertEqual(record["type"], "RuntimeError")
                    self.assertIn("Windows" if operation == "platform" else "window",
                                  record["message"])
                    self.assertIsNone(record["errno"])

    def test_actual_child_failure_remains_the_wait_failure_with_observer_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with child(root, "fail") as (process, trace):
                deadline = time.monotonic() + FIXTURE_SECONDS
                observation = startup.StartupObservation(process.pid, deadline, root)
                observation.sample()
                process.stdin.write(b"x")
                process.stdin.flush()
                with self.assertRaisesRegex(RuntimeError, "exits before calculated"):
                    scenario.wait_event(process, trace, "calculated", deadline,
                                        observation=observation)
                observation.finish()
                self.assertEqual(process.returncode, 1)
                self.assertIn("ZeroDivisionError", (root / "child.stderr").read_text())
                self.assertEqual(trace.read_bytes(), b"")
                self.assertEqual(json.loads((root / "startup.json").read_text()),
                                 observation.report)
                self.assertGreaterEqual(sum(v["count"] for v in
                                            observation.report["observer_errors"].values()), 1)

    def test_expired_actual_deadline_starts_no_native_observation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with child(root) as (process, trace):
                deadline = time.monotonic()
                observation = startup.StartupObservation(process.pid, deadline, root)
                with self.assertRaisesRegex(TimeoutError, "strict fixture deadline"):
                    scenario.wait_event(process, trace, "calculated", deadline,
                                        observation=observation)
                observation.finish()
                self.assertEqual(observation.report["attempts"], 0)
                self.assertEqual(observation.report["observer_errors"], {})
                self.assertEqual(observation.report["deadline_monotonic"], deadline)
                self.assertIsNone(process.poll())
                self.assertFalse((root / "startup.bmp").exists())

    def test_compiled_observer_has_no_input_focus_or_readiness_authority(self):
        path = SCRIPTS / "webview_acceptance/startup.py"
        source = path.read_text()
        compile(source, str(path), "exec")
        tree = ast.parse(source)
        calls = {node.func.attr for node in ast.walk(tree)
                 if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)}
        self.assertEqual(calls & {"focus_window", "send_virtual_key", "send_unicode_text",
                                  "send_pointer_click", "SetFocus", "PrintWindow"}, set())
        self.assertIn("capture_screen_window", calls)
        self.assertIn("window_for_processes", calls)
        self.assertNotIn("page_ready", source)
        gdi_path = SCRIPTS / "native_capture/windows/gdi.py"
        gdi_source = gdi_path.read_text()
        compile(gdi_source, str(gdi_path), "exec")
        capture = next(node for node in ast.parse(gdi_source).body
                       if isinstance(node, ast.FunctionDef)
                       and node.name == "capture_screen_window")
        self.assertEqual([ast.unparse(node) for node in capture.decorator_list],
                         ["physical_coordinates()"])
        operations = [node for node in ast.walk(capture) if isinstance(node, ast.Call)
                      and isinstance(node.func, ast.Attribute)]
        copied = next(node for node in operations if node.func.attr == "BitBlt")
        returned = next(node for node in operations if node.func.attr == "DesktopFrame")
        self.assertEqual([ast.dump(value) for value in returned.args[:4]],
                         [ast.dump(copied.args[6].args[0]), ast.dump(copied.args[7].args[0]),
                          ast.dump(copied.args[3]), ast.dump(copied.args[4])])


if __name__ == "__main__":
    unittest.main()
