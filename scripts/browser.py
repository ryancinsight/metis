"""Build the browser WASM host and generated wasm-bindgen loader."""
from __future__ import annotations

import argparse
import functools
import json
import os
import pathlib
import shutil
import subprocess

from cargo_overlay import isolated_cargo
from verification import workspace

PHYSICAL_ROOT = pathlib.Path(__file__).resolve().parents[1]
ROOT = pathlib.Path(os.environ.get("METIS_NEUTRAL_ROOT", PHYSICAL_ROOT))
OUTPUT = ROOT / "output" / "browser"
SOURCE = ROOT / "examples" / "browser"
CONTENT_SECURITY_POLICY = ROOT / "crates" / "metis-core" / "src" / "content_security_policy.txt"
# Consumer module admission reads the canonical compiled Rust budget.
MAX_CONSUMER_PAGE_BYTES = 512 * 1024


def run(command: list[str]) -> None:
    subprocess.run(command, cwd=ROOT, check=True, timeout=300)


def cargo(arguments: list[str], *, timeout: int = 300, **options) -> subprocess.CompletedProcess:
    """Run Cargo against the committed lock, outside an inherited stack configuration.

    The stack overlay patches first-party providers to local trees, which cannot
    agree with the locked revisions, so an in-place `--locked` build fails before
    it compiles. `isolated_cargo` supplies a directory outside that configuration
    chain; the manifest is therefore passed explicitly.
    """
    with isolated_cargo(ROOT) as (directory, environment):
        return subprocess.run(
            ["cargo", *arguments, "--manifest-path", str(ROOT / "Cargo.toml")],
            cwd=directory,
            env=environment,
            check=True,
            timeout=timeout,
            **options,
        )


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
            result = subprocess.run(
                [str(candidate), "--version"],
                cwd=ROOT,
                check=True,
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


@functools.lru_cache(maxsize=1)
def metis_command() -> pathlib.Path:
    """Return Cargo's actual compiled CLI artifact from its structured output."""
    result = cargo(["build", "--locked", "--package", "metis-cli", "--bin", "metis",
                    "--message-format=json-render-diagnostics"], capture_output=True, text=True)
    artifacts = []
    for line in result.stdout.splitlines():
        record = json.loads(line)
        if (record.get("reason") == "compiler-artifact"
                and record.get("target", {}).get("name") == "metis"
                and record.get("target", {}).get("kind") == ["bin"]
                and record.get("executable")):
            artifacts.append(pathlib.Path(record["executable"]))
    if len(artifacts) != 1 or not artifacts[0].is_file():
        raise SystemExit("Cargo must report one existing Metis CLI executable")
    return artifacts[0]


@functools.lru_cache(maxsize=1)
def asset_budgets() -> dict[str, int]:
    """Read actual core-owned limits through its canonical Rust CLI command."""
    result = subprocess.run([str(metis_command()), "asset-budgets"], check=True,
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
        result = subprocess.run([str(metis_command()), "build", str(ROOT / "metis-browser.json"),
                                 str(distribution)], cwd=directory, env=environment,
                                check=True, text=True, capture_output=True, timeout=300)
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
    parser.add_argument("command", choices=("build",))
    parser.add_argument(
        "--consumer-package",
        type=pathlib.Path,
        help="Include a bounded wasm-bindgen consumer package for the supplied gallery",
    )
    parser.add_argument(
        "--consumer-gallery",
        type=pathlib.Path,
        help="Include a consumer gallery directory containing gallery.html, gallery.js and gallery.css",
    )
    args = parser.parse_args()
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
    main()
