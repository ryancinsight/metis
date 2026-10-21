"""Build the opt-in Rust selection example through canonical consumer packaging."""
from __future__ import annotations

import argparse
import json
import pathlib
import subprocess

import browser


def build() -> None:
    browser.cargo([
        "build", "--locked", "-p", "metis-web", "--example", "series_selection",
        "--target", "wasm32-unknown-unknown", "--release",
    ])
    metadata = browser.cargo(
        ["metadata", "--no-deps", "--format-version", "1", "--locked"],
        timeout=30, capture_output=True, text=True,
    )
    target = pathlib.Path(json.loads(metadata.stdout)["target_directory"])
    artifact = target / "wasm32-unknown-unknown" / "release" / "examples" / "series_selection.wasm"
    package = browser.ROOT / "output" / "series-selection-package"
    package.mkdir(parents=True, exist_ok=True)
    subprocess.run([
        browser.wasm_bindgen(), str(artifact), "--target", "web", "--no-typescript",
        "--out-dir", str(package),
    ], cwd=browser.ROOT, check=True, timeout=120)
    browser.build()
    gallery = browser.ROOT / "crates" / "metis-web" / "examples" / "series_selection" / "frontend"
    browser.package_consumer(package, gallery)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build",))
    parser.parse_args()
    build()
