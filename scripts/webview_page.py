"""Build or verify the packaged WebView2 loader and Rust WebAssembly module."""

from __future__ import annotations

import argparse
import json
import os
import tempfile
import pathlib
import shutil
import subprocess

from cargo_overlay import isolated_cargo
from verification import workspace

ROOT = pathlib.Path(__file__).resolve().parents[1]
GENERATED = ROOT / "crates" / "metis-app" / "src" / "frontend" / "webview" / "generated"
CRATE = "metis-webview-page"

def run(command: list[str]) -> None:
    subprocess.run(command, cwd=ROOT, check=True, timeout=300)


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


def build(*, check: bool) -> None:
    # The manifest stays rlib-only so its public contracts are a normal API
    # surface for the workspace lint gates; this single WebAssembly build
    # artifact asks for the loadable cdylib crate type explicitly.
    with isolated_cargo(ROOT) as (directory, environment):
        subprocess.run(
            ["cargo", "rustc", "--locked", "-p", CRATE, "--target", "wasm32-unknown-unknown",
             "--release", "--crate-type", "cdylib", "--manifest-path", str(ROOT / "Cargo.toml"),
             "--", "-C", "opt-level=z"],
            cwd=directory, env=environment, check=True, timeout=300,
        )
    bindgen = wasm_bindgen(ROOT)
    module = target_directory(ROOT) / "wasm32-unknown-unknown" / "release" / (
        "metis_webview_page.wasm"
    )
    with tempfile.TemporaryDirectory() as staging:
        working = pathlib.Path(staging)
        run([bindgen, str(module), "--target", "web", "--out-name", "page", "--out-dir", staging])
        wasm = (working / "page_bg.wasm").read_bytes()
        if not wasm.startswith(b"\0asm\x01\0\0\0"):
            raise SystemExit("wasm-bindgen emitted an invalid WebAssembly header")
        glue = (working / "page.js").read_text(encoding="utf-8")
        # The generated initializer resolves page_bg.wasm relative to the loader.
        assets = {"page.js": (glue + "\nawait __wbg_init();\n").encode(), "page_bg.wasm": wasm}
        for name, content in assets.items():
            path = GENERATED / name
            if check:
                if not path.is_file() or path.read_bytes() != content:
                    raise SystemExit(f"{name} differs from a clean regeneration")
            else:
                path.write_bytes(content)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build", "check"))
    arguments = parser.parse_args()
    build(check=arguments.command == "check")


if __name__ == "__main__":
    main()
