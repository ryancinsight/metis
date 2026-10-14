"""One bounded owned application or real Rust fixture per worker process."""

import json
import os
from pathlib import Path
import subprocess
import time

from native_capture.windows import processes
from native_input.windows import focus, keyboard, queue
from python_native_accessibility import UIA_POLL_MILLISECONDS, WEBVIEW2_FOCUS_PRIMER_TABS

from . import evidence, fixtures, journey, runtime
from .contract import CALCULATIONS, FIXTURE_SECONDS, MAX_TRACE_BYTES
from .startup import StartupObservation


def wait_event(process, trace: Path, name: str, deadline: float, predicate=None,
               *, observation: StartupObservation | None = None) -> dict:
    while time.monotonic() < deadline:
        selected = evidence.events(evidence.records(trace, complete=False), name)
        if predicate is not None:
            selected = [value for value in selected if predicate(value)]
        if len(selected) > 1:
            raise ValueError(f"actual native fixture emits duplicate {name}")
        if selected:
            return selected[0]
        if process.poll() is not None:
            raise RuntimeError(f"actual native process exits before {name}")
        if observation is not None:
            observation.sample()
        remaining = deadline - time.monotonic()
        if remaining > 0:
            time.sleep(min(remaining, UIA_POLL_MILLISECONDS / 1000))
    raise TimeoutError(f"actual native process does not emit {name} within its strict fixture deadline")


def run(name: str, executable: Path, fixture_executable: Path, root: Path,
        output: Path, module_digest: str) -> dict:
    from .uia import Uia

    deadline = time.monotonic() + FIXTURE_SECONDS
    output.mkdir(parents=True, exist_ok=False)
    trace = output / "native.jsonl"
    stdout_path = output / "native.stdout"
    report = {"schema": 1, "scenario": name, "status": "running", "timeout_seconds": FIXTURE_SECONDS}
    environment = os.environ.copy()
    environment.pop("METIS_WEBVIEW_ASSET_INVENTORY", None)
    if name in fixtures.FIXTURES:
        arguments = [str(fixture_executable), "--exact", fixtures.FIXTURES[name],
                     "--ignored", "--nocapture", "--test-threads=1"]
    elif name == "form":
        arguments = [str(executable), "--metis-webview", *CALCULATIONS[0].arguments()]
    elif name == "permissions":
        arguments = [str(executable), "--metis-webview-permission-probe", *CALCULATIONS[0].arguments()]
    elif name == "permissions_capture":
        arguments = [str(executable), "--metis-webview-permission-probe-capture",
                     str(output / "permissions.png"), *CALCULATIONS[0].arguments()]
    else:
        raise ValueError(f"unknown actual native scenario: {name}")
    process = None
    uia = None
    observation = None
    caught = None
    try:
        with stdout_path.open("wb") as stdout, trace.open("wb") as stderr:
            process = subprocess.Popen(arguments, cwd=root, env=environment,
                                       stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr)
            observation = StartupObservation(process.pid, deadline, output)
            report["startup_observation"] = observation.report
            interactive = name in ("form", "permissions", "correlated_terminal_rejections",
                                   "stop_remount_destruction")
            expected = "fixture_input_required" if interactive and name in fixtures.FIXTURES else "page_ready"
            actual = wait_event(process, trace, expected, deadline, observation=observation)
            observation.finish()
            if name in ("form", "permissions", "correlated_terminal_rejections", "stop_remount_destruction"):
                bounds = processes.wait_for_process_window(process)
                report["runtime"] = runtime.version(process.pid, deadline)
                if name in ("form", "permissions", "correlated_terminal_rejections", "stop_remount_destruction"):
                    focus.focus_window(bounds.handle)
                    for _ in range(WEBVIEW2_FOCUS_PRIMER_TABS):
                        keyboard.send_virtual_key(0x09, key_up=False)
                        keyboard.send_virtual_key(0x09, key_up=True)
                        queue.flush_window(bounds.handle)
                    uia = Uia(bounds.handle, deadline, output)
                    if name == "form":
                        report["journey"] = journey.form(uia, output)
                    elif name == "permissions":
                        wait_event(process, trace, "page_probe_completed", deadline,
                                   lambda value: value.get("reconciled") is True)
                        uia.wait("result", lambda node: node["text"] == "Completed 4 bounded capability probes.")
                        report["journey"] = {"capture": journey.captured(uia, output / "permissions.png")}
                        uia.key(0x1B)
                        report["journey"]["actions"] = uia.actions
                    elif name == "stop_remount_destruction":
                        if actual.get("mount_id") != "1":
                            raise ValueError("actual visible remount fixture begins at the wrong mount")
                        mounts = [journey.remount(uia, actual)]
                        for mount in ("2", "3"):
                            requested = wait_event(process, trace, "fixture_input_required", deadline,
                                                   lambda value: value.get("mount_id") == mount)
                            mounts.append(journey.remount(uia, requested))
                        report["journey"] = {"mounts": mounts, "actions": uia.actions}
                    else:
                        report["journey"] = journey.correlated(uia, actual)
                    uia.close()
                    uia = None
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("actual native fixture exceeds its strict deadline")
            report["returncode"] = process.wait(timeout=remaining)
        if report["returncode"] != 0:
            raise RuntimeError(f"actual native {name} exits with {report['returncode']}; see {trace}")
        values = evidence.records(trace)
        if name in fixtures.FIXTURES:
            if stdout_path.stat().st_size > MAX_TRACE_BYTES:
                raise ValueError("actual fixture standard output exceeds its evidence budget")
            report["fixture"] = fixtures.verify(name, values, stdout_path.read_text(encoding="utf-8"), module_digest)
        else:
            report["lifecycle"] = evidence.lifecycle(values, module_digest)
            if name in ("permissions", "permissions_capture"):
                report["permissions"] = evidence.permissions(values)
                if not (output / "permissions.png").is_file():
                    raise ValueError("native permission capture is absent after successful lifecycle")
            else:
                actual_results = [value for value in report["journey"]["values"] if "dom" in value]
                report["calculations"] = [evidence.calculation(values, case, observed["dom"])
                                          for case, observed in zip(CALCULATIONS, actual_results)]
        report["status"] = "passed"
        return report
    except BaseException as error:
        caught = error
        report["status"] = "failed"
        report["error"] = f"{type(error).__name__}: {error}"
        if trace.exists() and trace.stat().st_size <= MAX_TRACE_BYTES:
            try:
                report["partial_native_trace"] = evidence.records(trace, complete=False)
                if name in ("permissions", "permissions_capture"):
                    report["permission_diagnostics"] = evidence.permission_diagnostics(report["partial_native_trace"])
            except (ValueError, UnicodeError) as parse_error:
                report["trace_parse_error"] = str(parse_error)
        raise
    finally:
        cleanup_errors = []
        if observation is not None:
            observation.finish()
        if uia is not None:
            report["partial_uia"] = {"actions": uia.actions, "last_nodes": uia.latest}
            try:
                uia.process.terminate()
                uia.process.wait(timeout=max(0, deadline - time.monotonic()))
            except (OSError, subprocess.TimeoutExpired) as error:
                cleanup_errors.append(f"owned UIA actor collection: {error}")
            uia.error.close()
        if process is not None and process.poll() is None:
            # Only the owned worker's failed process is terminated here. The
            # outer held-bootstrap Job Object collects all remaining descendants.
            try:
                process.terminate()
                process.wait(timeout=max(0, deadline - time.monotonic()))
            except (OSError, subprocess.TimeoutExpired) as error:
                cleanup_errors.append(f"owned native process collection: {error}")
            report["forced_parent_termination"] = True
        if cleanup_errors:
            report["status"] = "failed"
            report["cleanup_errors"] = cleanup_errors
            if caught is not None:
                caught.add_note("; ".join(cleanup_errors))
        (output / "evidence.json").write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8")
        if cleanup_errors and caught is None:
            raise RuntimeError("; ".join(cleanup_errors))
