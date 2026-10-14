"""Build the browser WASM host and generated wasm-bindgen loader.

The dependency order is CLI compilation -> Rust asset budgets -> gallery
admission. The explicit tool producer and every consumer resolve the same
locked Cargo artifact through one owner. Verification bounds compilation
separately from consumer execution; consumers still perform normal Cargo
freshness validation. No guessed executable path or injected tool skips it.
"""
from __future__ import annotations

import argparse
import functools
import hashlib
import json
import os
import pathlib
import shutil
import signal
import subprocess
import sys
from typing import NamedTuple

from cargo_overlay import isolated_cargo
from verification import workspace

PHYSICAL_ROOT = pathlib.Path(__file__).resolve().parents[1]
ROOT = pathlib.Path(os.environ.get("METIS_NEUTRAL_ROOT", PHYSICAL_ROOT))
OUTPUT = ROOT / "output" / "browser"
SOURCE = ROOT / "examples" / "browser"
CONTENT_SECURITY_POLICY = ROOT / "crates" / "metis-core" / "src" / "content_security_policy.txt"
# Consumer module admission reads the canonical compiled Rust budget.
MAX_CONSUMER_PAGE_BYTES = 512 * 1024


def run(command: list[str], *, cwd=ROOT, timeout=300, **options) -> subprocess.CompletedProcess:
    """Preserve checked-tool failure status, context, and both captured streams.

    Captured Rust tool text uses UTF-8 independently of the Windows console
    code page. Raw capture remains raw. Emit both streams before propagating
    the original CalledProcessError; callers retain its argv/status/payloads.
    """
    if options.get("text"):
        options.setdefault("encoding", "utf-8")
    try:
        return subprocess.run(command, cwd=cwd, check=True, timeout=timeout, **options)
    except subprocess.CalledProcessError as error:
        stream = sys.stderr.buffer
        context = {"command": error.cmd, "cwd": str(cwd), "returncode": error.returncode}
        stream.write(("browser subprocess failed: " + json.dumps(context, ensure_ascii=False) + "\n").encode("utf-8"))
        for name, data in (("stdout", error.stdout), ("stderr", error.stderr)):
            stream.write((name + ":\n").encode("utf-8"))
            if data is not None:
                stream.write(data.encode("utf-8") if isinstance(data, str) else data)
            stream.write(b"\n")
        stream.flush()
        raise


def terminate(returncode: int) -> None:
    """Preserve the checked child's process exit or POSIX termination signal.

    POSIX subprocess returncode -s means termination by signal s, not an
    integer exit status. Default disposition, an unblocked signal and delivery
    to this process reproduce that wait status; SystemExit(-s) would truncate
    it to an unrelated unsigned exit byte. Nonnegative exits stay unchanged.
    """
    if os.name != "posix" or returncode >= 0:
        raise SystemExit(returncode)
    number = -returncode
    if number not in (signal.SIGKILL, signal.SIGSTOP):
        signal.signal(number, signal.SIG_DFL)
    signal.pthread_sigmask(signal.SIG_UNBLOCK, {number})
    os.kill(os.getpid(), number)


class CargoExecution(NamedTuple):
    """Immutable actual process output and the selected Cargo execution context."""
    argv: tuple[str, ...]
    cwd: pathlib.Path
    rustup_toolchain: str
    returncode: int
    stdout: str | bytes | None
    stderr: str | bytes | None


def cargo(arguments: list[str], *, timeout: int = 300, **options) -> CargoExecution:
    """Run the one lock-authoritative Cargo owner outside incompatible patches.

    General browser fixtures and the CLI producer use this actual context and
    process owner. Capture options preserve their native text/byte contracts;
    every retained field is immutable, including argv and captured streams.
    """
    with isolated_cargo(ROOT) as (directory, environment):
        command = ("cargo", *arguments, "--manifest-path", str(ROOT / "Cargo.toml"))
        result = run(list(command), cwd=directory, env=environment, timeout=timeout, **options)
        return CargoExecution(command, directory, environment.get("RUSTUP_TOOLCHAIN", ""),
                              result.returncode, result.stdout, result.stderr)


def wasm_bindgen() -> str:
    required = workspace.package_version(ROOT / "Cargo.lock", "wasm-bindgen")
    configured = os.environ.get("WASM_BINDGEN")
    candidates = (
        pathlib.Path(configured) if configured else None,
        ROOT / "output" / "wasm-bindgen-cli" / "bin" / "wasm-bindgen.exe",
        pathlib.Path(shutil.which("wasm-bindgen") or ""),
    )
    for candidate in candidates:
        if candidate and candidate.is_file():
            result = run(
                [str(candidate), "--version"],
                cwd=ROOT,
                capture_output=True,
                text=True,
                timeout=30,
            )
            if result.stdout.strip() == f"wasm-bindgen {required}":
                return str(candidate)
            raise SystemExit(
                f"wasm-bindgen {required} is required; found {result.stdout.strip()}"
            )
    raise SystemExit(
        f"wasm-bindgen CLI is required; install version {required} or set WASM_BINDGEN to its path"
    )


class CompiledTool(NamedTuple):
    """Immutable executable identity and original compiler production evidence."""
    executable: pathlib.Path
    executable_sha256: str
    artifact_json: str
    cargo_argv: tuple[str, ...]
    cargo_cwd: pathlib.Path
    rustup_toolchain: str
    lock_sha256: str
    compiler_stdout: str
    compiler_stderr: str


def _digest(path: pathlib.Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def _compiler_streams(stdout: str, stderr: str) -> None:
    stream = sys.stderr.buffer
    stream.write(b"Cargo compiler stdout:\n" + stdout.encode("utf-8")
                 + b"\nCargo compiler stderr:\n" + stderr.encode("utf-8"))
    stream.flush()


@functools.lru_cache(maxsize=1)
def compile_tool() -> CompiledTool:
    """Compile and retain the one actual CLI artifact under the committed lock.

    The original JSON line, strings and tuple fields cannot be mutated through
    the cached value. isolated_cargo preserves the caller's established Rustup
    selector and target while avoiding incompatible ancestor source patches.
    """
    result = cargo(["build", "--locked", "--package", "metis-cli", "--bin", "metis",
                    "--message-format=json-render-diagnostics"], capture_output=True, text=True)
    artifacts = []
    try:
        for line in result.stdout.splitlines():
            record = json.loads(line)
            if (record.get("reason") == "compiler-artifact"
                    and record.get("target", {}).get("name") == "metis"
                    and record.get("target", {}).get("kind") == ["bin"]
                    and record.get("executable")):
                artifacts.append((pathlib.Path(record["executable"]), line))
    except (ValueError, TypeError):
        _compiler_streams(result.stdout, result.stderr)
        raise
    if len(artifacts) != 1 or not artifacts[0][0].is_file():
        _compiler_streams(result.stdout, result.stderr)
        raise SystemExit("Cargo must report one existing Metis CLI executable")
    executable, artifact = artifacts[0]
    return CompiledTool(executable, _digest(executable), artifact, result.argv, result.cwd,
                        result.rustup_toolchain, _digest(ROOT / "Cargo.lock"), result.stdout, result.stderr)


@functools.lru_cache(maxsize=1)
def asset_budgets() -> dict[str, int]:
    """Read actual core-owned limits through its canonical Rust CLI command."""
    result = run([str(compile_tool().executable), "asset-budgets"],
                            capture_output=True, text=True, cwd=ROOT, timeout=30)
    value = json.loads(result.stdout)
    expected = {"schema", "portable_path_bytes", "records", "manifest_bytes", "module_bytes", "payload_bytes"}
    if set(value) != expected or value["schema"] != 1:
        raise SystemExit("Rust asset budget report has an unknown schema")
    if any(type(item) is not int or item <= 0 for item in value.values()):
        raise SystemExit("Rust asset budget report has an invalid domain value")
    return value


def content_security_policy() -> str:
    policy = CONTENT_SECURITY_POLICY.read_text(encoding="utf-8")
    if not policy or policy != policy.strip() or any(char in policy for char in '\r\n"'):
        raise SystemExit("content-security policy source must be one quoted-free line")
    return policy


def validate_index_policy(index: pathlib.Path) -> None:
    document = index.read_text(encoding="utf-8")
    marker = 'http-equiv="Content-Security-Policy" content="'
    if marker not in document:
        raise SystemExit("browser index is missing its content-security policy")
    actual = document.split(marker, 1)[1].split('"', 1)[0]
    expected = content_security_policy()
    if actual != expected:
        raise SystemExit("browser index content-security policy differs from HostPolicy source")


def build() -> None:
    """The Rust CLI owns compilation, generation, staging and its inventory."""
    validate_index_policy(SOURCE / "index.html")
    distribution = OUTPUT.parent / "browser-distribution"
    with isolated_cargo(ROOT) as (directory, environment):
        environment["WASM_BINDGEN"] = wasm_bindgen()
        result = run([str(compile_tool().executable), "build", str(ROOT / ".config" / "browser" / "metis.json"),
                                 str(distribution)], cwd=directory, env=environment,
                                text=True, capture_output=True, timeout=300)
    served = pathlib.Path(result.stdout.strip())
    if served != distribution / "app" or not served.is_dir():
        raise SystemExit("Rust browser build does not report its exact served directory")
    OUTPUT.mkdir(parents=True, exist_ok=True)
    # Preserve the diagnostic server's public output path. Admission and the
    # build inventory belong to the Rust producer; diagnostics copy its bytes.
    shutil.copytree(served, OUTPUT, dirs_exist_ok=True)
    required = ("index.html", "styles.css", "bootstrap.js", "metis_web.js", "metis_web_bg.wasm",
                "assets/metis-mark.png", "assets/metis-mark.ico", "assets/metis-mark.svg")
    missing = [name for name in required if not (OUTPUT / name).is_file()]
    if missing:
        raise SystemExit(f"Rust browser build omits required artifacts: {missing}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("tool", help="Compile the CLI prerequisite and retain its artifact evidence")
    build_command = commands.add_parser("build", help="Build the browser application")
    build_command.add_argument(
        "--consumer-package",
        type=pathlib.Path,
        help="Include a bounded wasm-bindgen consumer package for the supplied gallery",
    )
    build_command.add_argument(
        "--consumer-gallery",
        type=pathlib.Path,
        help="Include a consumer gallery directory containing gallery.html, gallery.js and gallery.css",
    )
    args = parser.parse_args()
    if args.command == "tool":
        tool = compile_tool()
        _compiler_streams(tool.compiler_stdout, tool.compiler_stderr)
        print(json.dumps({"schema": 1, "executable": str(tool.executable),
                          "executable_sha256": tool.executable_sha256,
                          "compiler_artifact": json.loads(tool.artifact_json),
                          "cargo_argv": tool.cargo_argv, "cargo_cwd": str(tool.cargo_cwd),
                          "rustup_toolchain": tool.rustup_toolchain, "lock_sha256": tool.lock_sha256,
                          "source_root": str(ROOT.resolve())}))
        return
    if args.command == "build":
        build()
        if (args.consumer_package is None) != (args.consumer_gallery is None):
            raise SystemExit("--consumer-package and --consumer-gallery must be supplied together")
        if args.consumer_package is not None and args.consumer_gallery is not None:
            package_consumer(args.consumer_package, args.consumer_gallery)


def package_consumer(package: pathlib.Path, gallery: pathlib.Path) -> None:
    """Copy one validated consumer package and its browser page."""
    if not package.is_dir() or package.is_symlink():
        raise SystemExit(f"consumer package must be a regular directory: {package}")
    package_entries = sorted(package.iterdir())
    modules = [path for path in package_entries if path.is_file()]
    if len(package_entries) != 2 or len(modules) != 2 or {path.suffix for path in modules} != {".js", ".wasm"}:
        raise SystemExit("consumer package must contain exactly one JavaScript and one WebAssembly module")
    for source in modules:
        if source.is_symlink() or source.stat().st_size > asset_budgets()["module_bytes"]:
            raise SystemExit(f"consumer package contains an invalid or oversized file: {source}")
    wasm = next(source for source in modules if source.suffix == ".wasm")
    with wasm.open("rb") as module:
        if module.read(8) != b"\0asm\x01\0\0\0":
            raise SystemExit(f"consumer package contains an invalid WebAssembly header: {wasm}")

    if not gallery.is_dir() or gallery.is_symlink():
        raise SystemExit(f"consumer gallery must be a regular directory: {gallery}")
    page_names = ("gallery.html", "gallery.js", "gallery.css")
    gallery_entries = sorted(gallery.iterdir())
    pages = [gallery / name for name in page_names]
    if {path.name for path in gallery_entries} != set(page_names) or len(gallery_entries) != len(page_names):
        raise SystemExit("consumer gallery must contain exactly gallery.html, gallery.js and gallery.css")
    for source in pages:
        if not source.is_file() or source.is_symlink() or source.stat().st_size > MAX_CONSUMER_PAGE_BYTES:
            raise SystemExit(f"consumer gallery contains an invalid or oversized page: {source}")
    validate_index_policy(pages[0])

    destination = OUTPUT / "consumer"
    destination.mkdir(parents=True, exist_ok=True)
    for source in modules:
        shutil.copy2(source, destination / source.name)
    for source in pages:
        shutil.copy2(source, OUTPUT / source.name)


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        terminate(error.returncode)
