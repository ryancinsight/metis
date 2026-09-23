"""Run the committed rasterizer benchmarks under a bounded wall-clock budget.

Benchmarks are local measurement instruments, never hosted-runner jobs: a
shared runner measures its own contention. This runner pins the timing process
to a fixed set of performance cores at raised priority so a concurrent build
cannot move a verdict, records the host load beside the result, and terminates
the suite if it exceeds the committed budget.
"""
import argparse
import json
import os
import pathlib
import platform
import subprocess
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from verify import neutral_workspace

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "output" / "bench"
TOOLCHAIN = "1.97.0"
# Committed suite-total bound. Eight cases at roughly 2.5 s each leave ample
# room; a breach is an oversized instrument or a slow system under test, never
# a reason to raise this number.
BUDGET_SECONDS = 300
# Performance cores reserved for timing. Core 0 services interrupts.
DEFAULT_CORES = (2, 3)
NEWLINE = chr(10)


def affinity_mask(cores):
    """Builds the processor affinity mask for the reserved cores."""
    mask = 0
    for core in cores:
        mask |= 1 << core
    return mask


def host_load():
    """Samples the concurrent load a wall-clock verdict must be read against."""
    if platform.system() != "Windows":
        return {"available": False, "reason": "load sampling is implemented for Windows"}
    probe = subprocess.run(
        ["powershell", "-NoProfile", "-NonInteractive", "-Command",
         "(Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage"
         " -Average).Average"],
        capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=60,
        check=False,
    )
    value = probe.stdout.strip()
    if probe.returncode != 0 or not value.isdigit():
        return {"available": False, "reason": "load probe returned no value"}
    return {"available": True, "average_percent": int(value), "cpu_count": os.cpu_count()}


def build(root, package, bench):
    """Compiles the bench target and returns its executable path."""
    completed = subprocess.run(
        ["rustup", "run", TOOLCHAIN, "cargo", "bench", "--manifest-path",
         str(root / "Cargo.toml"), "--locked", "-p", package, "--bench", bench, "--no-run",
         "--message-format", "json"],
        cwd=root, capture_output=True, text=True, encoding="utf-8", errors="replace",
        timeout=BUDGET_SECONDS * 4, check=False,
    )
    if completed.returncode != 0:
        sys.stderr.write(completed.stderr)
        raise SystemExit(f"bench build failed with exit {completed.returncode}")
    for line in completed.stdout.splitlines():
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        if record.get("reason") == "compiler-artifact" and record.get("executable"):
            if record["target"]["name"] == bench:
                return pathlib.Path(record["executable"])
    raise SystemExit(f"bench build produced no executable for {bench}")


def measure(root, executable, cores, extra):
    """Runs the pinned bench process and returns its captured output."""
    creation = subprocess.CREATE_NEW_PROCESS_GROUP if platform.system() == "Windows" else 0
    started = time.monotonic()
    with subprocess.Popen(
        [str(executable), "--bench", *extra], cwd=root, stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT, text=True, encoding="utf-8", errors="replace",
        creationflags=creation,
    ) as process:
        if platform.system() == "Windows":
            try:
                import ctypes

                handle = ctypes.windll.kernel32.OpenProcess(0x0200 | 0x0400, False, process.pid)
                if handle:
                    ctypes.windll.kernel32.SetProcessAffinityMask(
                        handle, affinity_mask(cores)
                    )
                    # ABOVE_NORMAL_PRIORITY_CLASS keeps the timing process off
                    # the scheduler queue behind ordinary background work.
                    ctypes.windll.kernel32.SetPriorityClass(handle, 0x00008000)
                    ctypes.windll.kernel32.CloseHandle(handle)
            except OSError as error:
                print(f"pinning unavailable: {error}", file=sys.stderr)
        else:
            os.sched_setaffinity(process.pid, set(cores))
        try:
            output = process.communicate(timeout=BUDGET_SECONDS)[0]
        except subprocess.TimeoutExpired:
            process.kill()
            process.communicate()
            raise SystemExit(
                f"bench suite exceeded its committed {BUDGET_SECONDS}s budget"
            ) from None
    return output, time.monotonic() - started, process.returncode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", default="metis-platform")
    parser.add_argument("--bench", default="rasterizer")
    parser.add_argument("--cores", default=",".join(str(core) for core in DEFAULT_CORES),
                        help="comma-separated reserved core indices")
    parser.add_argument("--smoke", action="store_true",
                        help="run one iteration per case instead of timing")
    arguments = parser.parse_args()
    # Criterion reports use typographic minus signs; the default Windows
    # console encoding cannot represent them.
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    cores = tuple(int(part) for part in arguments.cores.split(",") if part)
    if not cores:
        raise SystemExit("at least one reserved core is required")

    before = host_load()
    with neutral_workspace() as neutral:
        root = pathlib.Path(neutral)
        executable = build(root, arguments.package, arguments.bench)
        output, seconds, code = measure(
            root, executable, cores, ["--test"] if arguments.smoke else []
        )
    after = host_load()
    sys.stdout.write(output)
    OUTPUT.mkdir(parents=True, exist_ok=True)
    report = {
        "schema": 1,
        "package": arguments.package,
        "bench": arguments.bench,
        "mode": "smoke" if arguments.smoke else "timing",
        "budget_seconds": BUDGET_SECONDS,
        "elapsed_seconds": round(seconds, 3),
        "reserved_cores": list(cores),
        "exit_code": code,
        "host_load_before": before,
        "host_load_after": after,
    }
    (OUTPUT / "latest.json").write_text(
        json.dumps(report, indent=2) + NEWLINE, encoding="utf-8"
    )
    if code != 0:
        raise SystemExit(f"bench run exited {code}")
    print(f"bench {arguments.bench} finished in {seconds:.1f}s of {BUDGET_SECONDS}s")


if __name__ == "__main__":
    main()
