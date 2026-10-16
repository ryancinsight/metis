"""Build the packaged WebView2 page loader (METIS-WEBVIEW-RUST-001).

The packaged page presentation is Rust owned by ``crates/metis-webview-page``
and compiled to WebAssembly. This script runs the audited chain — cargo build
under the committed lock, the pinned ``wasm-bindgen-cli`` — and writes the
packaged loader with one mechanical auto-init suffix into
``crates/metis-app/src/frontend/webview/generated/page.js``.

The suffix exists because the packaged page runs from a ``file://`` origin,
where the browser cannot fetch sibling files: the loader therefore embeds the
module bytes and calls the generated ``initSync`` with them. The suffix is a
build artifact, not authored page code, and `check` verifies the committed
loader byte-for-byte against a clean regeneration.
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import pathlib
import shutil
import subprocess

from cargo_overlay import isolated_cargo
from verification import workspace

PHYSICAL_ROOT = pathlib.Path(__file__).resolve().parents[1]
ROOT = pathlib.Path(__file__).resolve().parents[1]
GENERATED = ROOT / "crates" / "metis-app" / "src" / "frontend" / "webview" / "generated"
LOADER = GENERATED / "page.js"
CRATE = "metis-webview-page"

AUTO_INIT_PROLOGUE = (
    "\n"
    "// Mechanical auto-init appended by scripts/webview_page.py. The packaged\n"
    "// page runs from a file:// origin, so it cannot fetch sibling modules: the\n"
    "// generator embeds the module bytes below and starts them synchronously.\n"
    "initSync({ module: new WebAssembly.Module(\n"
    "  Uint8Array.from(atob(\"{ENCODED}\"), (b) => b.charCodeAt(0))) });\n"
)


def run(command: list[str]) -> None:
    subprocess.run(command, cwd=ROOT, check=True, timeout=300)


def cargo(arguments: list[str], *, timeout: int = 300) -> subprocess.CompletedProcess:
    """Run Cargo against the committed lock, outside an inherited stack
    configuration, like ``scripts/browser.py``."""
    with isolated_cargo(ROOT) as (directory, environment):
        return subprocess.run(
            ["cargo", *arguments, "--manifest-path", str(ROOT / "Cargo.toml")],
            cwd=directory,
            env=environment,
            check=True,
            timeout=timeout,
            **{"capture_output": False},
        )


def wasm_bindgen(root: pathlib.Path) -> str:
    required = workspace.package_version(root / "Cargo.lock", "wasm-bindgen")
    configured = os.environ.get("WASM_BINDGEN")
    candidates = (
        pathlib.Path(configured) if configured else None,
        root / "output" / "wasm-bindgen-cli" / "bin" / "wasm-bindgen.exe",
        pathlib.Path(shutil.which("wasm-bindgen") or ""),
    )
    for candidate in candidates:
        if not candidate or not candidate.is_file():
            continue
        result = subprocess.run(
            [str(candidate), "--version"],
            cwd=root,
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
        f"wasm-bindgen {required} is required; install it with "
        "cargo install wasm-bindgen-cli --version "
        f"{required} --locked"
    )


def target_directory(root: pathlib.Path) -> pathlib.Path:
    with isolated_cargo(root) as (directory, environment):
        result = subprocess.run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked",
             "--manifest-path", str(root / "Cargo.toml")],
            cwd=directory,
            env=environment,
            check=True,
            capture_output=True,
            text=True,
            timeout=30,
        )
    return pathlib.Path(json.loads(result.stdout)["target_directory"])


def render_loader(glue: str, module: bytes) -> str:
    """Append the mechanical auto-init suffix carrying the module bytes."""
    encoded = base64.b64encode(module).decode("ascii")
    return glue + AUTO_INIT_PROLOGUE.replace("{ENCODED}", encoded)


def build() -> None:
    # The manifest stays rlib-only so its public contracts are a normal API
    # surface for the workspace lint gates; this single WebAssembly build
    # artifact asks for the loadable cdylib crate type explicitly.
    cargo(
        ["rustc", "--locked", "-p", CRATE, "--target", "wasm32-unknown-unknown",
         "--release", "--crate-type", "cdylib"]
    )
    bindgen = wasm_bindgen(ROOT)
    module = target_directory(ROOT) / "wasm32-unknown-unknown" / "release" / (
        "metis_webview_page.wasm"
    )
    working = GENERATED / ".build"
    if working.exists():
        shutil.rmtree(working)
    run([bindgen, str(module), "--target", "web", "--out-name", "page", "--out-dir", str(working)])
    glue = (working / "page.js").read_text(encoding="utf-8")
    wasm = (working / "page_bg.wasm").read_bytes()
    if wasm[:8] != b"\0asm\x01\0\0\0":
        raise SystemExit("wasm-bindgen emitted an invalid WebAssembly header")
    shutil.rmtree(working)
    if not LOADER.parent.exists():
        LOADER.parent.mkdir(parents=True)
    LOADER.write_text(render_loader(glue, wasm), encoding="utf-8")
    print(f"webview_page_loader={LOADER.relative_to(ROOT)} bytes={LOADER.stat().st_size}")


def check() -> None:
    if not LOADER.is_file():
        raise SystemExit("the committed page loader is missing; run scripts/webview_page.py build")
    before = LOADER.read_bytes()
    build()
    after = LOADER.read_bytes()
    if before != after:
        raise SystemExit(
            "the committed page loader differs from a clean regeneration; "
            "rebuild and commit the loader"
        )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build", "check"))
    arguments = parser.parse_args()
    if arguments.command == "build":
        build()
    elif arguments.command == "check":
        check()


if __name__ == "__main__":
    main()
