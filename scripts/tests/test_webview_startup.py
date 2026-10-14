"""Real process/clock evidence and read-only startup ownership contracts."""

import ast
from contextlib import contextmanager
import errno
import json
import os
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
import time
import unittest

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))

from webview_acceptance import cases, dispatch, retention, scenario, startup
from webview_acceptance.contract import FIXTURE_SECONDS, MAX_TRACE_BYTES


class NativeObservationSelection(unittest.TestCase):
    def test_only_form_backend_parents_receive_the_exact_private_selection(self):
        selection = "MOIRAI_WEBVIEW_STARTUP_OBSERVATION"
        controlled = {selection: "inherited selection must not propagate",
                      "METIS_WEBVIEW_ASSET_INVENTORY": "outside producer ownership",
                      "PRESERVED": "λ🦀"}
        expected = {"start_uri": "https://metis.invalid/index.html",
                    "selectors": ["#host-status", "#result"]}
        program = """
import json, os
value = os.environ.get('MOIRAI_WEBVIEW_STARTUP_OBSERVATION')
print(json.dumps({'selection': json.loads(value) if value is not None else None,
                 'inventory': 'METIS_WEBVIEW_ASSET_INVENTORY' in os.environ,
                 'preserved': os.environ['PRESERVED']}))
"""
        names = ("form", "permissions", "permissions_capture", *scenario.fixtures.FIXTURES)
        for name in names:
            with self.subTest(scenario=name, observed=False):
                projected = scenario.observation_environment(name, controlled)
                self.assertEqual(projected.get("PRESERVED"), controlled["PRESERVED"])
                self.assertNotIn("METIS_WEBVIEW_ASSET_INVENTORY", projected)
                self.assertNotIn(selection, projected)
                self.assertEqual(controlled[selection], "inherited selection must not propagate")
                environment = scenario.observation_environment(name, {**os.environ, **controlled})
                observed = subprocess.run([sys.executable, "-c", program], env=environment,
                                          capture_output=True, check=True, timeout=FIXTURE_SECONDS)
                self.assertEqual(json.loads(observed.stdout), {
                    "selection": None,
                    "inventory": False, "preserved": controlled["PRESERVED"]})
            with self.subTest(scenario=name, observed=True):
                if name != "form":
                    with self.assertRaisesRegex(ValueError, "only.*form"):
                        scenario.observation_environment(name, controlled, observe_startup=True)
                    continue
                environment = scenario.observation_environment(
                    name, {**os.environ, **controlled}, observe_startup=True)
                observed = subprocess.run([sys.executable, "-c", program], env=environment,
                                          capture_output=True, check=True, timeout=FIXTURE_SECONDS)
                self.assertEqual(json.loads(observed.stdout), {
                    "selection": expected, "inventory": False,
                    "preserved": controlled["PRESERVED"]})

    def test_case_plan_requires_both_full_form_roles_and_exclusive_input_waves(self):
        actual = [(case.identifier, case.role, case.observe_startup)
                  for wave in cases.WAVES for case in wave]
        self.assertEqual(actual, [
            ("form", "form", True), ("form_ordinary", "form", False),
            ("permissions", "permissions", False),
            ("permissions_capture", "permissions_capture", False),
            ("malformed_duplicate_stale_commands", "malformed_duplicate_stale_commands", False),
            ("stop_remount_destruction", "stop_remount_destruction", False),
            ("correlated_terminal_rejections", "correlated_terminal_rejections", False),
        ])
        self.assertEqual(len({case.identifier for wave in cases.WAVES for case in wave}), 7)
        self.assertEqual(cases.SCHEDULE_SECONDS, 490)
        caller = ast.parse((SCRIPTS / "verification/windows/__init__.py").read_text())
        command = next(node for node in ast.walk(caller) if isinstance(node, ast.Call)
                       and isinstance(node.func, ast.Name) and node.func.id == "execute"
                       and node.args and isinstance(node.args[0], ast.Constant)
                       and node.args[0].value == "windows-webview-acceptance")
        outer_seconds = next(ast.literal_eval(keyword.value) for keyword in command.keywords
                             if keyword.arg == "seconds")
        self.assertEqual(outer_seconds - cases.SCHEDULE_SECONDS, 50)
        self.assertEqual([case.role for wave in cases.WAVES for case in wave
                          if case.role in cases.INPUT_ROLES],
                         ["form", "form", "permissions", "stop_remount_destruction",
                          "correlated_terminal_rejections"])
        for wave in cases.WAVES:
            if any(case.role in cases.INPUT_ROLES for case in wave):
                self.assertEqual(len(wave), 1)
            for case in wave:
                report = {"scenario": case.role,
                          "provider_startup_observation": cases.provider_selection(
                              case.role, case.observe_startup)}
                case.admit(report)
                self.assertEqual(report["provider_startup_observation"]["selected"], case.observe_startup)
                for invalid in ({**report, "scenario": "outside"},
                                {**report, "provider_startup_observation": None},
                                {**report, "provider_startup_observation": {
                                    "selected": not case.observe_startup, "configuration": None}},
                                {**report, "provider_startup_observation": {
                                    "selected": int(case.observe_startup), "configuration": None}}):
                    with self.assertRaisesRegex(ValueError, "role/observation"):
                        case.admit(invalid)
        selected, ordinary = cases.WAVES[0][0], cases.WAVES[1][0]
        self.assertEqual(selected.arguments(), ("--worker", "form", "--observe-startup"))
        self.assertEqual(ordinary.arguments(), ("--worker", "form"))
        with self.assertRaisesRegex(ValueError, "boolean"):
            scenario.observation_environment("form", {}, observe_startup="true")

    def test_actual_worker_options_reject_invalid_observation_before_native_admission(self):
        entry = SCRIPTS / "webview_acceptance.py"
        parse = runpy.run_path(str(entry))["arguments"]
        with tempfile.TemporaryDirectory() as directory:
            base = [value for name in ("executable", "fixture-executable", "inventory", "root", "output")
                    for value in ("--" + name, directory)] + ["--revision", "actual-option-contract"]
            for role in cases.ROLES:
                actual = parse([*base, "--worker", role])
                self.assertEqual(actual.worker, role)
                self.assertFalse(actual.observe_startup)
            self.assertTrue(parse([*base, "--worker", "form", "--observe-startup"]).observe_startup)
            for invalid, message in (
                (["--observe-startup"], "only with --worker form"),
                (["--worker", "permissions", "--observe-startup"], "only with --worker form"),
                (["--worker", "outside"], "invalid choice"),
                (["--worker", "form", "--observe-startup=false"], "ignored explicit argument"),
            ):
                with self.subTest(arguments=invalid):
                    actual = subprocess.run([sys.executable, str(entry), *base, *invalid],
                                            capture_output=True, text=True, timeout=FIXTURE_SECONDS)
                    self.assertEqual(actual.returncode, 2)
                    self.assertIn(message, actual.stderr)
                    self.assertNotIn("actual Windows acceptance requires", actual.stderr)
                    self.assertEqual(actual.stdout, "")


class RealProcessWaves(unittest.TestCase):
    def test_real_native_log_close_errors_preserve_body_error_and_close_every_stream(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for fails_in_body in (True, False):
                with self.subTest(body_failure=fails_in_body):
                    paths = tuple(root / f"{fails_in_body}-{index}.log" for index in range(2))
                    original = None
                    expected = ZeroDivisionError if fails_in_body else OSError
                    with self.assertRaises(expected) as caught:
                        with retention.streams(paths) as actual:
                            actual[0].write(b"pending actual data")
                            actual[1].write(str(256 * 257 // 2).encode())
                            os.close(actual[0].fileno())
                            if fails_in_body:
                                try:
                                    1 // 0
                                except ZeroDivisionError as error:
                                    original = error
                                    raise
                    self.assertEqual([stream.closed for stream in actual], [True, True])
                    self.assertEqual(paths[1].read_text(), str(256 * 257 // 2))
                    if fails_in_body:
                        self.assertIs(caught.exception, original)
                        self.assertIn("actual native log close failure", caught.exception.__notes__[0])
                    else:
                        self.assertEqual(caught.exception.errno, errno.EBADF)
                        self.assertIn(str(paths[0]), caught.exception.__notes__[0])

    def test_real_stream_budget_is_inclusive_and_rejects_excess_before_persistence(self):
        program = ("import sys; size=int(sys.argv[1])-2; "
                   "sys.stdout.write('λ'*(size//2)+'x'*(size%2)); sys.stderr.write('Ω')")
        sizes = (MAX_TRACE_BYTES - 1, MAX_TRACE_BYTES, MAX_TRACE_BYTES + 1)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            wave = dispatch.run(tuple((sys.executable, "-c", program, str(size)) for size in sizes),
                                root, {**os.environ, "PYTHONUTF8": "1"})
            retained = retention.processes(wave.outcomes, root, ("model-0", "model-1", "model-2"),
                                           byte_limit=MAX_TRACE_BYTES)
            self.assertEqual([len((actual.stdout + actual.stderr).encode()) for actual in wave.outcomes],
                             list(sizes))
            for index in (0, 1):
                actual = wave.outcomes[index]
                self.assertEqual((root / f"model-{index}.log").read_bytes(),
                                 (actual.stdout + actual.stderr).encode())
                self.assertEqual(retained.records[index]["returncode"], 0)
            self.assertFalse((root / "model-2.log").exists())
            self.assertFalse((root / "model-2.stdout").exists())
            self.assertFalse((root / "model-2.stderr").exists())
            self.assertEqual(retained.records[2]["bytes"], MAX_TRACE_BYTES + 1)
            self.assertEqual(retained.records[2]["limit_bytes"], MAX_TRACE_BYTES)
            self.assertEqual(len(retained.errors), 1)
            with self.assertRaisesRegex(ValueError, "evidence exceeds its byte budget") as caught:
                wave.require_success(retained.errors)
            self.assertIs(caught.exception, retained.errors[0])

    def test_real_persistence_failures_preserve_child_identity_and_all_later_streams(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            commands = ((str(root / "invalid\0executable"),),
                        (sys.executable, "-c", "import pathlib,sys; value=sum(range(257)); "
                         "pathlib.Path(sys.argv[1]).write_text(str(value)); "
                         "print(value); print('completed λ',file=sys.stderr)", str(root / "complete")))
            wave = dispatch.run(commands, root, {**os.environ, "PYTHONUTF8": "1"})
            original, completed = wave.outcomes
            self.assertIsInstance(original.error, ValueError)
            self.assertEqual(completed.returncode, 0)
            self.assertEqual((root / "complete").read_text(), str(256 * 257 // 2))
            blocked = root / "blocked"
            blocked.write_text("actual regular file")
            try:
                (blocked / "first.stdout").write_text("actual native write admission")
            except OSError as error:
                native = error
            else:
                self.fail("native filesystem unexpectedly admits a child of a regular file")
            retained = retention.processes(wave.outcomes, root, ("blocked/first", "later"))
            self.assertEqual(len(retained.records), 2)
            self.assertEqual(len(retained.errors), 3)
            for extension, error in zip(("stdout", "stderr", "log"), retained.errors, strict=True):
                self.assertEqual(type(error), type(native))
                self.assertEqual(error.errno, native.errno)
                self.assertEqual(Path(error.filename), blocked / f"first.{extension}")
                self.assertIn("retaining actual owned process stream", error.__notes__[0])
            self.assertEqual(blocked.read_text(), "actual regular file")
            self.assertEqual((root / "later.stdout").read_text(), completed.stdout)
            self.assertEqual((root / "later.stderr").read_text(), completed.stderr)
            self.assertEqual((root / "later.log").read_text(), completed.stdout + completed.stderr)
            self.assertEqual(int(completed.stdout), 256 * 257 // 2)
            with self.assertRaises(ValueError) as caught:
                wave.require_success(retained.errors)
            self.assertIs(caught.exception, original.error)
            self.assertEqual(len(caught.exception.__notes__), 3)
            report = {"value": int(completed.stdout)}
            report_path = blocked / "evidence.json"
            retention.report(report_path, report, original.error)
            self.assertIn("retaining actual evidence report", original.error.__notes__[-1])
            self.assertIn(str(report_path), original.error.__notes__[-1])
            with self.assertRaises(type(native)) as write_error:
                retention.report(report_path, report, None)
            self.assertEqual(write_error.exception.errno, native.errno)
            self.assertEqual(Path(write_error.exception.filename), report_path)

    def test_real_parallel_owners_compute_in_reverse_order_and_return_declared_values(self):
        program = """
import json, os, pathlib, sys, time
root, index = pathlib.Path(sys.argv[1]), int(sys.argv[2])
deadline = time.monotonic() + int(sys.argv[3])
(root / f'{index}.ready').write_text(str(os.getpid()))
while not all((root / f'{value}.ready').is_file() for value in range(3)):
    if time.monotonic() >= deadline: raise TimeoutError('real wave rendezvous')
    time.sleep(0)
if index < 2:
    while not (root / f'{index + 1}.done').is_file():
        if time.monotonic() >= deadline: raise TimeoutError('real successor completion')
        time.sleep(0)
value = sum(range((index + 1) * 17))
(root / f'{index}.done').write_text(json.dumps({'value': value, 'completed': time.monotonic_ns()}))
print(json.dumps({'index': index, 'pid': os.getpid(), 'value': value}))
print(f'actual stderr {index} λ', file=sys.stderr)
"""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            commands = tuple((sys.executable, "-c", program, str(root), str(index), str(FIXTURE_SECONDS))
                             for index in range(3))
            wave = dispatch.run(commands, root, {**os.environ, "PYTHONUTF8": "1"})
            retained = retention.processes(wave.outcomes, root, tuple(f"case-{index}" for index in range(3)))
            wave.require_success(retained.errors)
            self.assertEqual(len(retained.records), 3)
            pids, computations = set(), []
            for index, actual in enumerate(wave.outcomes):
                value = (index + 1) * 17
                expected = value * (value - 1) // 2
                computed = json.loads(actual.stdout)
                self.assertEqual(computed, {"index": index, "pid": int((root / f"{index}.ready").read_text()),
                                            "value": expected})
                completion = json.loads((root / f"{index}.done").read_text())
                self.assertEqual(completion["value"], expected)
                computations.append(completion["completed"])
                self.assertEqual(actual.command, commands[index])
                self.assertEqual((root / f"case-{index}.stderr").read_text(), f"actual stderr {index} λ\n")
                self.assertEqual(retained.records[index]["returncode"], 0)
                pids.add(computed["pid"])
            self.assertEqual(len(pids), 3)
            self.assertEqual(sorted(range(3), key=computations.__getitem__), [2, 1, 0])

    def test_real_failed_commands_keep_partial_streams_drain_success_and_raise_original_error(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            programs = (
                (sys.executable, "-c", "print('failure stdout λ', flush=True); "
                 "import sys; print('failure stderr 🦀',file=sys.stderr,flush=True); 1//0"),
                (str(root / "invalid\0executable"),),
                (sys.executable, "-c", "import pathlib,sys; value=sum(range(257)); "
                 "pathlib.Path(sys.argv[1]).write_text(str(value)); print(value)", str(root / "complete")),
            )
            wave = dispatch.run(programs, root, {**os.environ, "PYTHONUTF8": "1"})
            retained = retention.processes(wave.outcomes, root, tuple(f"failure-{index}" for index in range(3)))
            failed, invalid, completed = wave.outcomes
            self.assertEqual(failed.returncode, 1)
            self.assertEqual(failed.stdout, "failure stdout λ\n")
            self.assertIn("failure stderr 🦀\n", failed.stderr)
            self.assertIn("ZeroDivisionError", failed.stderr)
            self.assertIsInstance(invalid.error, ValueError)
            self.assertEqual(retained.records[1]["error"], f"ValueError: {invalid.error}")
            self.assertEqual(completed.returncode, 0)
            self.assertEqual(int(completed.stdout), 256 * 257 // 2)
            self.assertEqual((root / "complete").read_text(), str(256 * 257 // 2))
            with self.assertRaises(subprocess.CalledProcessError) as caught:
                wave.require_success()
            self.assertEqual(caught.exception.output, failed.stdout)
            self.assertEqual(caught.exception.stderr, failed.stderr)
            self.assertEqual(caught.exception.cmd, programs[0])
            self.assertIn("ValueError", caught.exception.__notes__[0])
            original = dispatch.Wave((invalid, completed))
            with self.assertRaises(ValueError) as caught:
                original.require_success()
            self.assertIs(caught.exception, invalid.error)


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
