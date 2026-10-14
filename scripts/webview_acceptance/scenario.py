"""One bounded owned application or real Rust fixture per worker process."""

from collections.abc import Mapping
import json
import os
from pathlib import Path
import subprocess
import time
import traceback

from native_capture.windows import processes
from native_input.windows import focus, keyboard, queue
from python_native_accessibility import UIA_POLL_MILLISECONDS, WEBVIEW2_FOCUS_PRIMER_TABS

from . import cases, evidence, fixtures, journey, retention, runtime
from .contract import CALCULATIONS, FIXTURE_SECONDS, MAX_TRACE_BYTES
from .startup import StartupObservation


def observation_environment(name: str, source: Mapping[str, str], *,
                            observe_startup: bool = False) -> dict[str, str]:
    """Strip inherited selection; only explicit form input enables diagnostics."""
    selection = cases.provider_selection(name, observe_startup)
    environment = dict(source)
    environment.pop("METIS_WEBVIEW_ASSET_INVENTORY", None)
    environment.pop(cases.SELECTION_ENV, None)
    if selection["selected"]:
        environment[cases.SELECTION_ENV] = json.dumps(selection["configuration"], separators=(",", ":"))
    return environment


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


def _journey(name, uia, actual, process, trace, deadline, output) -> dict:
    """Execute the existing role's full native-input journey exactly once."""
    if name == "form":
        return journey.form(uia, output)
    if name == "permissions":
        wait_event(process, trace, "page_probe_completed", deadline,
                   lambda value: value.get("reconciled") is True)
        uia.wait("result", lambda node: node["text"] == "Completed 4 bounded capability probes.")
        observed = {"capture": journey.captured(uia, output / "permissions.png")}
        uia.key(0x1B)
        observed["actions"] = uia.actions
        return observed
    if name == "stop_remount_destruction":
        if actual.get("mount_id") != "1":
            raise ValueError("actual visible remount fixture begins at the wrong mount")
        mounts = [journey.remount(uia, actual)]
        for mount in ("2", "3"):
            requested = wait_event(process, trace, "fixture_input_required", deadline,
                                   lambda value: value.get("mount_id") == mount)
            mounts.append(journey.remount(uia, requested))
        return {"mounts": mounts, "actions": uia.actions}
    return journey.correlated(uia, actual)


def _collect(process, uia, observation, deadline, output, report, caught) -> None:
    """Collect every owned native component without replacing its original error."""
    cleanup_errors = []
    completed_journey = report.get("status") == "passed"
    if completed_journey:
        report["status"] = "collecting"
    def checkpoint(stage):
        report["cleanup_stage"] = stage
        try:
            retention.report(output / "evidence.json", report, caught)
        except BaseException as error:
            error.add_note(f"retaining cleanup boundary: {stage}")
            cleanup_errors.append(error)

    checkpoint("entered")
    if observation is not None:
        checkpoint("startup_observer_finish")
        try:
            observation.finish()
        except BaseException as error:
            error.add_note("finalizing actual startup observation")
            cleanup_errors.append(error)
    if uia is not None:
        report["partial_uia"] = {"actions": uia.actions, "last_nodes": uia.latest}
        checkpoint("uia_terminate_and_wait")
        try:
            uia.process.terminate()
            uia.process.wait(timeout=max(0, deadline - time.monotonic()))
        except BaseException as error:
            error.add_note("owned UIA actor collection")
            cleanup_errors.append(error)
    if process is not None:
        # Only the owned worker's failed process is terminated here. The
        # outer held-bootstrap Job Object collects all remaining descendants.
        try:
            checkpoint("native_process_poll_terminate_and_wait")
            if process.poll() is None:
                report["forced_parent_termination"] = True
                process.terminate()
                process.wait(timeout=max(0, deadline - time.monotonic()))
        except BaseException as error:
            error.add_note("owned native process collection")
            cleanup_errors.append(error)
    if cleanup_errors:
        report["status"] = "failed"
        report["cleanup_errors"] = [f"{type(error).__name__}: {error}; {'; '.join(error.__notes__)}"
                                    for error in cleanup_errors]
    elif completed_journey:
        report["status"] = "passed"
    primary = caught if caught is not None else cleanup_errors[0] if cleanup_errors else None
    if primary is not None:
        for error in cleanup_errors:
            if error is not primary:
                primary.add_note(f"additional actual cleanup failure: {type(error).__name__}: {error}; "
                                 f"{'; '.join(error.__notes__)}")
    report["cleanup_stage"] = "attempts_finished" if cleanup_errors else "complete"
    retention.report(output / "evidence.json", report, primary)
    if caught is None and primary is not None:
        raise primary


def run(name: str, executable: Path, fixture_executable: Path, root: Path,
        output: Path, module_digest: str, *, observe_startup: bool = False) -> dict:
    from .uia import Uia

    deadline = time.monotonic() + FIXTURE_SECONDS
    output.mkdir(parents=True, exist_ok=False)
    trace = output / "native.jsonl"
    stdout_path = output / "native.stdout"
    report = {"schema": 1, "scenario": name, "status": "running", "timeout_seconds": FIXTURE_SECONDS,
              "provider_startup_observation": cases.provider_selection(name, observe_startup)}
    environment = observation_environment(name, os.environ, observe_startup=observe_startup)
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
        with retention.streams((stdout_path, trace)) as (stdout, stderr):
            process = subprocess.Popen(arguments, cwd=root, env=environment,
                                       stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr)
            observation = StartupObservation(process.pid, deadline, output)
            report["startup_observation"] = observation.report
            interactive = name in cases.INPUT_ROLES
            expected = "fixture_input_required" if interactive and name in fixtures.FIXTURES else "page_ready"
            actual = wait_event(process, trace, expected, deadline, observation=observation)
            observation.finish()
            if interactive:
                bounds = processes.wait_for_process_window(process)
                report["runtime"] = runtime.version(process.pid, deadline)
                if interactive:
                    focus.focus_window(bounds.handle)
                    for _ in range(WEBVIEW2_FOCUS_PRIMER_TABS):
                        keyboard.send_virtual_key(0x09, key_up=False)
                        keyboard.send_virtual_key(0x09, key_up=True)
                        queue.flush_window(bounds.handle)
                    uia = Uia(bounds.handle, deadline, output)
                    report["journey"] = _journey(name, uia, actual, process, trace, deadline, output)
                    uia.close()
                    uia = None
            report["stage"] = "native_exit_wait"
            retention.report(output / "evidence.json", report, None)
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
                                          for case, observed in zip(CALCULATIONS, actual_results, strict=True)]
        report["status"] = "passed"
        return report
    except BaseException as error:
        caught = error
        report["status"] = "failed"
        report["error"] = f"{type(error).__name__}: {error}"
        report["error_traceback"] = traceback.format_exception(error)
        try:
            if trace.exists() and trace.stat().st_size <= MAX_TRACE_BYTES:
                report["partial_native_trace"] = evidence.records(trace, complete=False)
                if name in ("permissions", "permissions_capture"):
                    report["permission_diagnostics"] = evidence.permission_diagnostics(report["partial_native_trace"])
        except BaseException as parse_error:
            report["trace_parse_error"] = f"{type(parse_error).__name__}: {parse_error}"
            error.add_note(f"retaining partial actual native trace: {type(parse_error).__name__}: {parse_error}")
        report["stage"] = "primary_failure_before_cleanup"
        retention.report(output / "evidence.json", report, error)
        raise
    finally:
        _collect(process, uia, observation, deadline, output, report, caught)
