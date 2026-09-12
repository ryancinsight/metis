"""Measure one real application lifecycle with bounded process-tree sampling.

The runner is deliberately framework-neutral. It records resource and lifecycle
evidence for a command supplied by the caller; it does not parse application
arguments or infer a framework ranking. Reports are local run artifacts and may
be kept under the repository's ignored ``output`` directory.
"""
from __future__ import annotations

import argparse
import ctypes
import hashlib
import math
import json
import os
import pathlib
import platform
import shutil
import signal
import subprocess
import sys
import time
from dataclasses import dataclass
from typing import Iterable, Mapping, Sequence


MAX_SAMPLES = 30_000
MAX_SAMPLE_INTERVAL_MS = 1_000
MAX_TIMEOUT_SECONDS = 300
NORMAL_95_CRITICAL = 1.96
PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
TH32CS_SNAPPROCESS = 0x00000002


@dataclass(frozen=True)
class ProcessSample:
    """One observation of the command's visible process tree."""

    elapsed_ms: int
    process_count: int
    working_set_bytes: int
    private_bytes: int | None
    handle_count: int | None


def command_fingerprint(command: Sequence[str]) -> str:
    """Hash the exact command without putting private paths or arguments in a report."""
    encoded = json.dumps(list(command), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _validate_output(path: pathlib.Path) -> pathlib.Path:
    """Reject redirected or multiply-linked report paths."""
    path = path.resolve()
    if path.exists() and (path.is_symlink() or path.stat().st_nlink != 1):
        raise ValueError(f"unsafe resource report path: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() and (path.is_symlink() or path.stat().st_nlink != 1):
        raise ValueError(f"unsafe resource report path: {path}")
    return path


def _children(root_pid: int) -> set[int]:
    """Return the root and descendants that are visible to this process."""
    if os.name == "nt":
        return _windows_children(root_pid)
    return _proc_children(root_pid)


def _proc_children(root_pid: int) -> set[int]:
    parents: dict[int, int] = {}
    proc = pathlib.Path("/proc")
    if not proc.is_dir():
        return {root_pid}
    for entry in proc.iterdir():
        if not entry.name.isdecimal():
            continue
        try:
            status = (entry / "status").read_text(encoding="ascii", errors="replace")
            parent = next(int(line.split(":", 1)[1]) for line in status.splitlines() if line.startswith("PPid:"))
            parents[int(entry.name)] = parent
        except (OSError, StopIteration, ValueError):
            continue
    descendants = {root_pid}
    changed = True
    while changed:
        changed = False
        for pid, parent in parents.items():
            if parent in descendants and pid not in descendants:
                descendants.add(pid)
                changed = True
    return descendants


if os.name == "nt":
    class _ProcessEntry(ctypes.Structure):
        _fields_ = [
            ("dwSize", ctypes.c_uint32),
            ("cntUsage", ctypes.c_uint32),
            ("th32ProcessID", ctypes.c_uint32),
            ("th32DefaultHeapID", ctypes.c_size_t),
            ("th32ModuleID", ctypes.c_uint32),
            ("cntThreads", ctypes.c_uint32),
            ("th32ParentProcessID", ctypes.c_uint32),
            ("pcPriClassBase", ctypes.c_int32),
            ("dwFlags", ctypes.c_uint32),
            ("szExeFile", ctypes.c_wchar * 260),
        ]

    class _MemoryCounters(ctypes.Structure):
        _fields_ = [
            ("cb", ctypes.c_uint32),
            ("PageFaultCount", ctypes.c_uint32),
            ("PeakWorkingSetSize", ctypes.c_size_t),
            ("WorkingSetSize", ctypes.c_size_t),
            ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
            ("QuotaPagedPoolUsage", ctypes.c_size_t),
            ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
            ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
            ("PagefileUsage", ctypes.c_size_t),
            ("PeakPagefileUsage", ctypes.c_size_t),
        ]

    def _windows_children(root_pid: int) -> set[int]:
        """Enumerate descendants through the Toolhelp process snapshot."""
        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        snapshot_call = kernel32.CreateToolhelp32Snapshot
        snapshot_call.argtypes = [ctypes.c_uint32, ctypes.c_uint32]
        snapshot_call.restype = ctypes.c_void_p
        snapshot = snapshot_call(TH32CS_SNAPPROCESS, 0)
        invalid = ctypes.c_void_p(-1).value
        if snapshot == invalid:
            return {root_pid}
        first = kernel32.Process32FirstW
        first.argtypes = [ctypes.c_void_p, ctypes.POINTER(_ProcessEntry)]
        first.restype = ctypes.c_bool
        following = kernel32.Process32NextW
        following.argtypes = [ctypes.c_void_p, ctypes.POINTER(_ProcessEntry)]
        following.restype = ctypes.c_bool
        close = kernel32.CloseHandle
        close.argtypes = [ctypes.c_void_p]
        parents: dict[int, int] = {}
        entry = _ProcessEntry(dwSize=ctypes.sizeof(_ProcessEntry))
        try:
            valid = first(snapshot, ctypes.byref(entry))
            while valid:
                parents[entry.th32ProcessID] = entry.th32ParentProcessID
                entry.dwSize = ctypes.sizeof(_ProcessEntry)
                valid = following(snapshot, ctypes.byref(entry))
        finally:
            close(snapshot)
        descendants = {root_pid}
        changed = True
        while changed:
            changed = False
            for pid, parent in parents.items():
                if parent in descendants and pid not in descendants:
                    descendants.add(pid)
                    changed = True
        return descendants


def _memory(pid: int) -> tuple[int, int | None, int | None]:
    """Read working set, private bytes and handles for one visible process."""
    if os.name == "nt":
        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        psapi = ctypes.WinDLL("psapi", use_last_error=True)
        open_process = kernel32.OpenProcess
        open_process.argtypes = [ctypes.c_uint32, ctypes.c_bool, ctypes.c_uint32]
        open_process.restype = ctypes.c_void_p
        handle = open_process(PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
        if not handle:
            return 0, None, None
        try:
            counters = _MemoryCounters(cb=ctypes.sizeof(_MemoryCounters))
            read = psapi.GetProcessMemoryInfo
            read.argtypes = [ctypes.c_void_p, ctypes.POINTER(_MemoryCounters), ctypes.c_uint32]
            read.restype = ctypes.c_bool
            if not read(handle, ctypes.byref(counters), counters.cb):
                return 0, None, None
            handles = ctypes.c_uint32()
            get_handles = kernel32.GetProcessHandleCount
            get_handles.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32)]
            get_handles.restype = ctypes.c_bool
            handle_count = int(handles.value) if get_handles(handle, ctypes.byref(handles)) else None
            return int(counters.WorkingSetSize), int(counters.PagefileUsage), handle_count
        finally:
            kernel32.CloseHandle(handle)
    status_path = pathlib.Path("/proc") / str(pid) / "status"
    try:
        values = {}
        for line in status_path.read_text(encoding="ascii", errors="replace").splitlines():
            key, separator, value = line.partition(":")
            if separator and key == "VmRSS":
                values[key] = int(value.strip().split()[0]) * 1024
        private_values = []
        rollup = pathlib.Path("/proc") / str(pid) / "smaps_rollup"
        if rollup.is_file():
            for line in rollup.read_text(encoding="ascii", errors="replace").splitlines():
                key, separator, value = line.partition(":")
                if separator and key in {"Private_Clean", "Private_Dirty"}:
                    private_values.append(int(value.strip().split()[0]) * 1024)
        handles_path = pathlib.Path("/proc") / str(pid) / "fd"
        handle_count = len(tuple(handles_path.iterdir())) if handles_path.is_dir() else None
        private_bytes = sum(private_values) if private_values else None
        return values.get("VmRSS", 0), private_bytes, handle_count
    except (OSError, ValueError):
        return 0, None, None


def sample_process_tree(root_pid: int, elapsed_ms: int) -> ProcessSample:
    """Collect one bounded aggregate observation for a process tree."""
    pids = _children(root_pid)
    measurements = [_memory(pid) for pid in pids]
    private_values = [value for _, value, _ in measurements if value is not None]
    handle_values = [value for _, _, value in measurements if value is not None]
    return ProcessSample(
        elapsed_ms=elapsed_ms,
        process_count=len(measurements),
        working_set_bytes=sum(value for value, _, _ in measurements),
        private_bytes=sum(private_values) if private_values else None,
        handle_count=sum(handle_values) if handle_values else None,
    )


def summarize(samples: Iterable[ProcessSample]) -> Mapping[str, object]:
    """Summarize samples without discarding missing platform measurements."""
    values = tuple(samples)
    if not values:
        return {"sample_count": 0, "working_set_bytes": {}, "private_bytes": {}, "handle_count": {}}

    def summary(field: str) -> Mapping[str, int | None]:
        present = [getattr(sample, field) for sample in values if getattr(sample, field) is not None]
        if not present:
            return {"initial": None, "final": None, "peak": None, "growth": None}
        return {
            "initial": int(present[0]),
            "final": int(present[-1]),
            "peak": int(max(present)),
            "growth": int(present[-1] - present[0]),
        }

    return {
        "sample_count": len(values),
        "elapsed_ms": {"initial": values[0].elapsed_ms, "final": values[-1].elapsed_ms},
        "process_count": {"initial": values[0].process_count, "peak": max(value.process_count for value in values)},
        "working_set_bytes": summary("working_set_bytes"),
        "private_bytes": summary("private_bytes"),
        "handle_count": summary("handle_count"),
    }


def _statistics(values: Iterable[int]) -> Mapping[str, float | int | None]:
    """Return a bounded sample spread and an explicitly approximate interval."""
    numbers = tuple(float(value) for value in values)
    if not numbers:
        return {"count": 0, "mean": None, "sample_stddev": None, "approximate_95_half_width": None}
    mean = sum(numbers) / len(numbers)
    if len(numbers) < 2:
        deviation = None
        half_width = None
    else:
        deviation = (sum((value - mean) ** 2 for value in numbers) / (len(numbers) - 1)) ** 0.5
        half_width = NORMAL_95_CRITICAL * deviation / (len(numbers) ** 0.5)
    return {
        "count": len(numbers),
        "mean": mean,
        "sample_stddev": deviation,
        "approximate_95_half_width": half_width,
    }


def aggregate_measurements(measurements: Iterable[Mapping[str, object]]) -> Mapping[str, object]:
    """Aggregate repeated runs while preserving missing counters as missing."""
    runs = tuple(measurements)
    scalar_fields = ("duration_ms", "startup_observation_ms")
    scalars = {
        field: _statistics(
            measurement[field] for measurement in runs
            if isinstance(measurement.get(field), (int, float))
        )
        for field in scalar_fields
    }
    summary_fields = ("working_set_bytes", "private_bytes", "handle_count")
    summary = {}
    for field in summary_fields:
        summary[field] = {}
        for phase in ("initial", "final", "peak", "growth"):
            summary[field][phase] = _statistics(
                measurement["summary"][field][phase]
                for measurement in runs
                if isinstance(measurement.get("summary"), Mapping)
                and isinstance(measurement["summary"].get(field), Mapping)
                and isinstance(measurement["summary"][field].get(phase), (int, float))
            )
    return {"run_count": len(runs), "scalars": scalars, "summary": summary}


def _terminate(process: subprocess.Popen[bytes]) -> None:
    """Terminate only the process launched by this measurement."""
    if process.poll() is not None:
        return
    if os.name == "nt":
        process.terminate()
    else:
        os.kill(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def run(command: Sequence[str], interval_ms: int, timeout_seconds: float,
        max_samples: int = MAX_SAMPLES) -> Mapping[str, object]:
    """Run and sample a command until exit or the explicit timeout."""
    started = time.perf_counter_ns()
    process = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    samples: list[ProcessSample] = []
    first_observation_ms: int | None = None
    deadline = started + int(timeout_seconds * 1_000_000_000)
    timed_out = False
    while True:
        now = time.perf_counter_ns()
        elapsed_ms = (now - started) // 1_000_000
        if process.poll() is not None:
            break
        if first_observation_ms is None:
            first_observation_ms = int(elapsed_ms)
        if len(samples) >= max_samples:
            timed_out = True
            break
        sample = sample_process_tree(process.pid, int(elapsed_ms))
        if process.poll() is not None:
            break
        samples.append(sample)
        if now >= deadline:
            timed_out = True
            break
        time.sleep(interval_ms / 1000)
    if timed_out:
        _terminate(process)
    process.wait()
    finished = time.perf_counter_ns()
    result = {
        "status": "timeout" if timed_out else ("passed" if process.returncode == 0 else "failed"),
        "exit_code": process.returncode,
        "duration_ms": (finished - started) // 1_000_000,
        "startup_observation_ms": first_observation_ms,
        "output_capture": "disabled",
        "summary": summarize(samples),
        "samples": [sample.__dict__ for sample in samples],
    }
    if timed_out:
        result["timeout_seconds"] = timeout_seconds
    return result


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=pathlib.Path, help="JSON report path")
    parser.add_argument("--label", required=True, help="Human-readable fixture label")
    parser.add_argument("--revision", default=None, help="Source revision supplied by the caller")
    parser.add_argument("--phase", default="lifecycle", choices=("startup", "idle", "active", "lifecycle"))
    parser.add_argument("--sample-ms", type=int, default=100, help="Sampling interval, 10-1000 ms")
    parser.add_argument("--timeout-seconds", type=float, default=60, help="Finite lifecycle timeout, 0.1-300 s")
    parser.add_argument("--repeat", type=int, default=1, help="Sequential runs for a fixture, 1-20")
    parser.add_argument("command", nargs=argparse.REMAINDER, help="Command after --")
    return parser


def main(arguments: Sequence[str] | None = None) -> int:
    """Validate arguments, run the process and write one report."""
    args = _parser().parse_args(arguments)
    command = list(args.command)
    if command and command[0] == "--":
        command.pop(0)
    if not command:
        raise SystemExit("a command is required after --")
    if not 10 <= args.sample_ms <= MAX_SAMPLE_INTERVAL_MS:
        raise SystemExit("--sample-ms must be between 10 and 1000")
    if not 0.1 <= args.timeout_seconds <= MAX_TIMEOUT_SECONDS:
        raise SystemExit("--timeout-seconds must be between 0.1 and 300")
    if not 1 <= args.repeat <= 20:
        raise SystemExit("--repeat must be between 1 and 20")
    if args.repeat * args.timeout_seconds > MAX_TIMEOUT_SECONDS:
        raise SystemExit("repeat count multiplied by timeout must not exceed 300 seconds")
    samples_per_run = max(1, MAX_SAMPLES // args.repeat)
    required_samples = math.ceil(args.timeout_seconds * 1000 / args.sample_ms) + 1
    if required_samples > samples_per_run:
        raise SystemExit("sampling budget is too small for the selected timeout and repeat count")
    executable = shutil.which(command[0]) or command[0]
    report_path = _validate_output(args.output)
    try:
        measurements = []
        for _ in range(args.repeat):
            measurement = run(command, args.sample_ms, args.timeout_seconds,
                              max_samples=samples_per_run)
            measurements.append(measurement)
            if measurement["status"] != "passed":
                break
        measurements = tuple(measurements)
    except OSError as error:
        report = {
            "schema": 1,
            "status": "launch-error",
            "label": args.label,
            "phase": args.phase,
            "platform": {"system": platform.system(), "release": platform.release()},
            "executable": pathlib.Path(executable).name,
            "argument_count": len(command) - 1,
            "command_sha256": command_fingerprint(command),
            "error_type": type(error).__name__,
        }
        report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        return 1
    statuses = {measurement["status"] for measurement in measurements}
    status = "timeout" if "timeout" in statuses else ("passed" if statuses == {"passed"} else "failed")
    report = {
        "schema": 1,
        "status": status,
        "label": args.label,
        "phase": args.phase,
        "platform": {"system": platform.system(), "release": platform.release(), "machine": platform.machine()},
        "python": platform.python_version(),
        "executable": pathlib.Path(executable).name,
        "argument_count": len(command) - 1,
        "command_sha256": command_fingerprint(command),
        "sample_interval_ms": args.sample_ms,
        "timeout_seconds": args.timeout_seconds,
        "repeat": args.repeat,
        "revision": args.revision,
        "measurements": measurements,
        "aggregate": aggregate_measurements(measurements),
    }
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
