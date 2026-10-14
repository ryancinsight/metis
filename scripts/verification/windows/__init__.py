"""Canonical Windows preparation, existing native capture and real page acceptance."""

import json
from pathlib import Path
import sys

from webview_acceptance.compiler import executable
from ..native_paths import contained_file
from . import archive, prepared


def prepare(execute, command, root: Path, output: Path, nonce: str) -> Path:
    """Run the actual release CLI before native check/build/test compilation."""
    compiled = execute("windows-cli-build", command([
        "build", "--release", "-p", "metis-cli", "--bin", "metis",
        "--message-format=json-render-diagnostics",
    ]))
    tool = executable(compiled, "metis-cli", "metis")
    directory = output / "browser" / "windows-webview-prepared" / nonce
    directory.parent.mkdir(parents=True, exist_ok=True)
    if directory.exists():
        raise ValueError("Windows asset preparation requires a fresh nonexistent directory")
    result = execute("windows-webview-prepare",
                     [str(tool), "prepare", str(root / "metis.json"), str(directory)])
    lines = result.splitlines()
    if len(lines) != 1:
        raise ValueError("canonical metis prepare must return only one inventory path")
    inventory = contained_file(Path(lines[0]), directory)
    budgets = json.loads(execute("windows-webview-budgets",
                                [str(tool), "asset-budgets"], seconds=30, cwd=root))
    prepared.record(inventory, directory, budgets)
    return inventory


def acceptance(execute, root: Path, output: Path, metadata: dict,
               revision: str, nonce: str, compiled: str, inventory: Path,
               fixture: Path) -> dict:
    """Retain the original host capture, then execute the complete real-page gate."""
    native_capture = Path(metadata["target_directory"]) / "debug" / "examples" / "native_host_capture.exe"
    native_output = output / "native-host"
    execute("native-host-capture", [str(native_capture), "--output", str(native_output),
                                   "--source-revision", revision], seconds=60, cwd=root)
    native_trace = json.loads((native_output / "native-host-trace.json").read_text(encoding="utf-8"))
    if native_trace.get("source_revision") != revision or native_trace.get("host_result") != "ok":
        raise ValueError("native-host capture is not tied to the verified revision")
    application = executable(compiled, "metis-app", "metis-app")
    destination = output / "browser" / "windows-webview" / nonce
    execute("windows-webview-acceptance", [
        sys.executable, str(root / "scripts" / "webview_acceptance.py"),
        "--executable", str(application), "--fixture-executable", str(fixture),
        "--inventory", str(inventory), "--root", str(root),
        "--output", str(destination), "--revision", revision,
    ], seconds=540, cwd=root)
    report = json.loads((destination / "evidence.json").read_text(encoding="utf-8"))
    if report.get("status") != "passed" or report.get("revision") != revision:
        raise ValueError("actual Windows Rust-page acceptance does not verify the current revision")
    return {"native_host": native_trace, "native_webview": report}
