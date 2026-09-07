"""Build the browser WASM host and generated wasm-bindgen loader."""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "output" / "browser"
WASM_BINDGEN_VERSION = "0.2.128"


def run(command: list[str]) -> None:
    subprocess.run(command, cwd=ROOT, check=True, timeout=300)


def wasm_bindgen() -> str:
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
            if result.stdout.strip() == f"wasm-bindgen {WASM_BINDGEN_VERSION}":
                return str(candidate)
            raise SystemExit(
                f"wasm-bindgen {WASM_BINDGEN_VERSION} is required; found {result.stdout.strip()}"
            )
    raise SystemExit(
        "wasm-bindgen CLI is required; install version 0.2.128 or set WASM_BINDGEN to its path"
    )


def wasm_artifact() -> pathlib.Path:
    metadata = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    target_directory = pathlib.Path(json.loads(metadata.stdout)["target_directory"])
    return target_directory / "wasm32-unknown-unknown" / "release" / "metis_web.wasm"


def build() -> None:
    run(["cargo", "build", "--locked", "-p", "metis-web", "--target", "wasm32-unknown-unknown", "--release"])
    OUTPUT.mkdir(parents=True, exist_ok=True)
    run([wasm_bindgen(), str(wasm_artifact()), "--target", "web", "--out-dir", str(OUTPUT)])
    shutil.copy2(ROOT / "examples" / "browser" / "index.html", OUTPUT / "index.html")
    for asset in ("styles.css", "bootstrap.js"):
        shutil.copy2(ROOT / "examples" / "browser" / asset, OUTPUT / asset)
    required = (
        OUTPUT / "index.html",
        OUTPUT / "styles.css",
        OUTPUT / "bootstrap.js",
        OUTPUT / "metis_web.js",
        OUTPUT / "metis_web_bg.wasm",
    )
    missing = [str(path) for path in required if not path.is_file()]
    if missing:
        raise SystemExit(f"browser build did not produce required artifacts: {', '.join(missing)}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build",))
    args = parser.parse_args()
    if args.command == "build":
        build()


if __name__ == "__main__":
    main()
