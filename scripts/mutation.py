"""Run the bounded protocol-decoder mutation suite."""

from __future__ import annotations

import hashlib
import json
import os
import pathlib
import signal
import shutil
import stat
import subprocess
import sys
import tomllib


ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "output" / "mutation"
LATEST = OUTPUT / "latest"
PREVIOUS = OUTPUT / "previous"
TOOLCHAIN_FILE = ROOT / "rust-toolchain.toml"
TOOL_VERSION = "27.1.0"
PACKAGE = "metis-core"
TEST_PACKAGE = "metis-ipc"
SOURCE = ROOT / "crates" / "metis-core" / "src" / "protocol" / "payload.rs"
EXAMINE = "decode"
TEST_TIMEOUT_SECONDS = 30
BUILD_TIMEOUT_SECONDS = 120
SUITE_TIMEOUT_SECONDS = 300
JOBS = 2


def _toolchain() -> str:
    """Read the workspace toolchain pin used by the normal gate."""
    configuration = tomllib.loads(TOOLCHAIN_FILE.read_text(encoding="utf-8"))
    channel = configuration.get("toolchain", {}).get("channel")
    if not isinstance(channel, str) or not channel:
        raise SystemExit("rust-toolchain.toml must declare a non-empty toolchain channel")
    return channel


def _target_directory(environment: dict[str, str]) -> pathlib.Path | None:
    """Resolve the existing shared Cargo target directory, when configured."""
    configured = environment.get("CARGO_TARGET_DIR")
    if configured:
        return pathlib.Path(configured).resolve()
    for directory in (ROOT, *ROOT.parents):
        for name in ("config", "config.toml"):
            path = directory / ".cargo" / name
            if not path.is_file():
                continue
            values = tomllib.loads(path.read_text(encoding="utf-8"))
            target = values.get("build", {}).get("target-dir")
            if isinstance(target, str) and target:
                return (path.parent.parent / target).resolve()
    return None


def _environment(toolchain: str) -> dict[str, str]:
    """Prepare a pinned, shared-cache environment without carrying secrets."""
    environment = os.environ.copy()
    if any(environment.get(name) for name in ("RUSTC", "RUSTDOC")):
        raise SystemExit("Unset RUSTC/RUSTDOC overrides; mutation analysis requires the pinned toolchain")
    environment["RUSTUP_TOOLCHAIN"] = toolchain
    environment["CARGO_INCREMENTAL"] = "0"
    tool_directory = ROOT / "output" / "cargo-tools" / "bin"
    if tool_directory.is_dir():
        environment["PATH"] = os.pathsep.join((str(tool_directory), environment.get("PATH", "")))
    target = _target_directory(environment)
    if target is not None:
        environment["CARGO_TARGET_DIR"] = str(target)
    return environment


def _assert_local_directory(path: pathlib.Path) -> None:
    """Reject output aliases before rotating derived mutation reports."""
    if not path.resolve().is_relative_to(ROOT):
        raise SystemExit(f"Mutation output escapes the repository: {path}")
    current = path
    while current != ROOT.parent:
        if current.exists():
            attributes = getattr(current.lstat(), "st_file_attributes", 0)
            if current.is_symlink() or attributes & stat.FILE_ATTRIBUTE_REPARSE_POINT:
                raise SystemExit(f"Mutation output follows a link: {current}")
        current = current.parent


def _prepare_output() -> None:
    """Rotate the fixed two-run report ring and create a clean latest directory."""
    _assert_local_directory(OUTPUT)
    _assert_local_directory(LATEST)
    _assert_local_directory(PREVIOUS)
    OUTPUT.mkdir(parents=True, exist_ok=True)
    if PREVIOUS.exists():
        if not PREVIOUS.is_dir():
            raise SystemExit(f"Mutation previous report is not a directory: {PREVIOUS}")
        shutil.rmtree(PREVIOUS)
    if LATEST.exists():
        if not LATEST.is_dir():
            raise SystemExit(f"Mutation latest report is not a directory: {LATEST}")
        shutil.move(str(LATEST), str(PREVIOUS))
    LATEST.mkdir()


def _git_value(arguments: list[str]) -> str:
    """Read a revision value with a finite process budget."""
    result = subprocess.run(
        ["git", *arguments],
        cwd=ROOT,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=30,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(f"git {' '.join(arguments)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def _base_report(toolchain: str, command: list[str]) -> dict[str, object]:
    """Create provenance for the exact source and command under test."""
    return {
        "schema": 1,
        "status": "running",
        "tool": {"name": "cargo-mutants", "version": TOOL_VERSION},
        "toolchain": toolchain,
        "revision": _git_value(["rev-parse", "HEAD"]),
        "tree": _git_value(["rev-parse", "HEAD^{tree}"]),
        "source": str(SOURCE.relative_to(ROOT)).replace("\\", "/"),
        "source_sha256": hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
        "package": PACKAGE,
        "test_package": TEST_PACKAGE,
        "examine": EXAMINE,
        "budgets": {
            "test_timeout_seconds": TEST_TIMEOUT_SECONDS,
            "build_timeout_seconds": BUILD_TIMEOUT_SECONDS,
            "suite_timeout_seconds": SUITE_TIMEOUT_SECONDS,
            "jobs": JOBS,
        },
        "command": command,
    }


def _write_report(report: dict[str, object]) -> None:
    """Persist the bounded report before and after the external run."""
    (LATEST / "manifest.json").write_text(
        json.dumps(report, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )


def summarize_outcomes(outcomes: pathlib.Path) -> dict[str, object]:
    """Extract mutation counts and the viable-mutant score from cargo-mutants."""
    values = json.loads(outcomes.read_text(encoding="utf-8"))
    counts = {
        key: values.get(key, 0)
        for key in ("total_mutants", "caught", "missed", "timeout", "unviable")
    }
    if not all(isinstance(value, int) and value >= 0 for value in counts.values()):
        raise SystemExit("cargo-mutants outcomes contain invalid non-negative counts")
    viable = counts["total_mutants"] - counts["unviable"]
    if viable < 0 or counts["caught"] > viable:
        raise SystemExit("cargo-mutants outcomes contain inconsistent viable-mutant counts")
    counts["viable_mutants"] = viable
    counts["score"] = counts["caught"] / viable if viable else None
    counts["cargo_mutants_version"] = values.get("cargo_mutants_version")
    if counts["cargo_mutants_version"] != TOOL_VERSION:
        raise SystemExit("cargo-mutants outcome version differs from the pinned tool")
    return counts


def _run(command: list[str], environment: dict[str, str]) -> tuple[int, str]:
    """Run cargo-mutants under one finite suite budget and retain its log."""
    creationflags = subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0
    process = subprocess.Popen(
        command,
        cwd=ROOT,
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        errors="replace",
        creationflags=creationflags,
    )
    try:
        stdout, stderr = process.communicate(timeout=SUITE_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired as error:
        if os.name == "nt":
            subprocess.run(
                ["taskkill", "/PID", str(process.pid), "/T", "/F"],
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                timeout=30,
                check=False,
            )
        else:
            os.killpg(process.pid, signal.SIGTERM)
        stdout, stderr = process.communicate(timeout=30)
        captured = "\n".join(
            value.decode(errors="replace") if isinstance(value, bytes) else value or ""
            for value in (error.stdout or stdout, error.stderr or stderr)
        )
        log = captured + f"\nmutation: exceeded {SUITE_TIMEOUT_SECONDS}-second budget\n"
        (LATEST / "cargo-mutants.log").write_text(log, encoding="utf-8")
        raise SystemExit(f"cargo-mutants exceeded {SUITE_TIMEOUT_SECONDS}-second budget") from error
    diagnostic = stdout + stderr
    (LATEST / "cargo-mutants.log").write_text(diagnostic, encoding="utf-8")
    return process.returncode, diagnostic


def main() -> None:
    """Run the decoder mutation slice and fail on a surviving viable mutant."""
    _prepare_output()
    toolchain = _toolchain()
    source = str(SOURCE.relative_to(ROOT)).replace("\\", "/")
    command = [
        "rustup",
        "run",
        toolchain,
        "cargo",
        "mutants",
        "--package",
        PACKAGE,
        "--test-package",
        TEST_PACKAGE,
        "--file",
        source,
        "--re",
        EXAMINE,
        "--test-tool",
        "nextest",
        "--timeout",
        str(TEST_TIMEOUT_SECONDS),
        "--build-timeout",
        str(BUILD_TIMEOUT_SECONDS),
        "--jobs",
        str(JOBS),
        "--no-shuffle",
        "--cargo-arg=--locked",
        "--cargo-arg=--offline",
        "--output",
        str(LATEST),
    ]
    report = _base_report(toolchain, command)
    _write_report(report)
    environment = _environment(toolchain)
    try:
        version_result = subprocess.run(
            ["rustup", "run", toolchain, "cargo", "mutants", "--version"],
            cwd=ROOT,
            env=environment,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=30,
            check=False,
        )
        version = version_result.stdout.strip()
        if version_result.returncode != 0 or version != f"cargo-mutants {TOOL_VERSION}":
            raise SystemExit(
                f"expected cargo-mutants {TOOL_VERSION}; install with "
                f"cargo install cargo-mutants --version {TOOL_VERSION} --locked --root output/cargo-tools"
            )
        report["tool_version_output"] = version
        _write_report(report)
        return_code, diagnostic = _run(command, environment)
        outcomes = LATEST / "mutants.out" / "outcomes.json"
        if not outcomes.is_file():
            raise SystemExit("cargo-mutants did not write mutants.out/outcomes.json")
        report["outcomes"] = summarize_outcomes(outcomes)
        report["exit_code"] = return_code
        if return_code != 0:
            report["status"] = "failed"
            _write_report(report)
            print(diagnostic[-12_000:], file=sys.stderr)
            raise SystemExit("cargo-mutants reported a surviving or timed-out mutant")
        report["status"] = "passed"
        _write_report(report)
        summary = report["outcomes"]
        assert isinstance(summary, dict)
        print(
            "Mutation slice passed: "
            f"{summary['caught']}/{summary['viable_mutants']} viable mutants caught; "
            f"{summary['unviable']} unviable"
        )
    except BaseException as error:
        report["status"] = "failed"
        report["error"] = str(error)
        _write_report(report)
        raise


if __name__ == "__main__":
    main()
