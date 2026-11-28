"""Build the browser WASM host and generated wasm-bindgen loader."""
from __future__ import annotations

import argparse
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
# Keep externally supplied browser assets bounded before they enter the output.
MAX_CONSUMER_MODULE_BYTES = 64 * 1024 * 1024
MAX_CONSUMER_PAGE_BYTES = 512 * 1024
HOST_EXPORTS = (
    "metis_start",
    "metis_stop",
    "metis_browser_start",
    "metis_browser_stop",
    "metis_http_start",
    "metis_http_stop",
)


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


def wasm_artifact(relative: str) -> pathlib.Path:
    metadata = cargo(
        ["metadata", "--no-deps", "--format-version", "1", "--locked"],
        timeout=30,
        capture_output=True,
        text=True,
    )
    target_directory = pathlib.Path(json.loads(metadata.stdout)["target_directory"])
    return target_directory / "wasm32-unknown-unknown" / "release" / relative


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


def check_artifact_exports(host: pathlib.Path, canvas: pathlib.Path) -> None:
    """Require application exports only in the distributed browser host."""
    def declarations(path: pathlib.Path) -> dict[str, str]:
        exports = {}
        for line in path.read_text(encoding="utf-8").splitlines():
            if line.startswith("export const "):
                name, separator, signature = line.removeprefix("export const ").partition(":")
                if not separator or name in exports:
                    raise SystemExit(f"generated export table is malformed: {line}")
                exports[name] = signature.strip()
        return exports

    host_exports = declarations(host)
    canvas_exports = declarations(canvas)

    def is_runtime_export(name: str) -> bool:
        if name in {"memory", "__wbindgen_add_to_stack_pointer", "__wbindgen_start"}:
            return True
        wbindgen_suffix = name.removeprefix("__wbindgen_export")
        if wbindgen_suffix != name and (
            not wbindgen_suffix or wbindgen_suffix.isdecimal()
        ):
            return True
        element_suffix = name.removeprefix("__wasm_bindgen_func_elem_")
        return element_suffix != name and all(
            part.isdecimal() for part in element_suffix.split("_")
        )

    for export in HOST_EXPORTS:
        if host_exports.get(export) != "() => void;":
            raise SystemExit(f"generated browser host is missing {export}")
    if canvas_exports.get("start") != "() => void;":
        raise SystemExit("generated canvas input is missing its start export")
    canvas_application_exports = {
        name for name in canvas_exports if not is_runtime_export(name)
    }
    unrelated = canvas_application_exports - {"start"}
    if unrelated:
        raise SystemExit(
            "generated canvas input retains unrelated " + ", ".join(sorted(unrelated))
        )


def build() -> None:
    cargo(["build", "--locked", "-p", "metis-web", "--target", "wasm32-unknown-unknown", "--release"])
    cargo([
        "build", "--locked", "-p", "metis-web", "--example", "canvas_input",
        "--target", "wasm32-unknown-unknown", "--release",
    ])
    OUTPUT.mkdir(parents=True, exist_ok=True)
    run([wasm_bindgen(), str(wasm_artifact("metis_web.wasm")), "--target", "web", "--out-dir", str(OUTPUT)])
    run([
        wasm_bindgen(), str(wasm_artifact("examples/canvas_input.wasm")), "--target", "web",
        "--out-dir", str(OUTPUT),
    ])
    canvas_loader = (OUTPUT / "canvas_input.js").read_text(encoding="utf-8")
    if "wasm.__wbindgen_start();" not in canvas_loader or "canvas_mount" in canvas_loader:
        raise SystemExit("generated canvas loader does not delegate startup to the Rust start hook")
    check_artifact_exports(
        OUTPUT / "metis_web_bg.wasm.d.ts",
        OUTPUT / "canvas_input_bg.wasm.d.ts",
    )
    index = SOURCE / "index.html"
    validate_index_policy(index)
    shutil.copy2(index, OUTPUT / "index.html")
    for asset in ("styles.css", "bootstrap.js"):
        shutil.copy2(SOURCE / asset, OUTPUT / asset)
    for source in sorted(SOURCE.rglob("*")):
        if not source.is_file() or source.name in {"index.html", "styles.css", "bootstrap.js"}:
            continue
        destination = OUTPUT / source.relative_to(SOURCE)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    required = (
        OUTPUT / "index.html",
        OUTPUT / "styles.css",
        OUTPUT / "bootstrap.js",
        OUTPUT / "metis_web.js",
        OUTPUT / "metis_web_bg.wasm",
        OUTPUT / "canvas_input.js",
        OUTPUT / "canvas_input_bg.wasm",
        OUTPUT / "canvas-input.html",
        OUTPUT / "canvas-input.js",
        OUTPUT / "assets" / "metis-mark.png",
        OUTPUT / "assets" / "metis-mark.ico",
        OUTPUT / "assets" / "metis-mark.svg",
    )
    missing = [str(path) for path in required if not path.is_file()]
    if missing:
        raise SystemExit(f"browser build did not produce required artifacts: {', '.join(missing)}")


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
        if source.is_symlink() or source.stat().st_size > MAX_CONSUMER_MODULE_BYTES:
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
