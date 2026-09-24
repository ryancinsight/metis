"""Build, serve and check the Metis starter application.

`build` compiles `crates/metis-starter` to WebAssembly, generates its loader
with wasm-bindgen and assembles the page under `output/browser/starter/`.
`serve` serves that page on loopback. `check` drives it through a W3C
WebDriver endpoint: it greets a name, verifies the reply Rust wrote, and can
capture the page.
"""
from __future__ import annotations

import argparse
import json
import pathlib
import shutil
import subprocess
import sys
import time

import browser
from browser_protocol import ROOT, BrowserRuntimeError, StaticServer, WebDriverClient

OUTPUT = ROOT / "output" / "browser" / "starter"
FRONTEND = ROOT / "crates" / "metis-starter" / "frontend"
FRONTEND_FILES = ("index.html", "styles.css", "main.js", "assets/webassembly.svg")
METIS_MARK = ROOT / "examples" / "browser" / "assets" / "metis-mark.svg"
MODULE = "metis_starter"
# The template's window size, so a capture matches the Tauri starter's frame.
WINDOW = (800, 600)
WAIT_SECONDS = 10
NAME = "Metis"
# Chromium's preferred-color-scheme setting: 1 selects light and 0 dark,
# measured against the page's prefers-color-scheme media query.
COLOR_SCHEMES = {"light": "--blink-settings=preferredColorScheme=1",
                 "dark": "--blink-settings=preferredColorScheme=0"}


def expected_greeting(name: str) -> str:
    """The reply `metis_starter::greet` returns; its doctest pins the wording."""
    return f"Hello, {name}! You've been greeted from Rust!"


def build() -> pathlib.Path:
    """Compile the module and assemble the served page; returns its directory."""
    browser.cargo([
        "build", "--locked", "-p", "metis-starter",
        "--target", "wasm32-unknown-unknown", "--release",
    ])
    metadata = browser.cargo(
        ["metadata", "--no-deps", "--format-version", "1", "--locked"],
        timeout=30, capture_output=True, text=True,
    )
    target = pathlib.Path(json.loads(metadata.stdout)["target_directory"])
    module = target / "wasm32-unknown-unknown" / "release" / f"{MODULE}.wasm"
    if not module.is_file():
        raise BrowserRuntimeError(f"Cargo did not produce the starter module: {module}")
    if OUTPUT.exists():
        shutil.rmtree(OUTPUT)
    (OUTPUT / "assets").mkdir(parents=True)
    subprocess.run(
        [browser.wasm_bindgen(), str(module), "--target", "web", "--out-dir", str(OUTPUT)],
        cwd=ROOT, check=True, timeout=300,
    )
    for relative in FRONTEND_FILES:
        shutil.copyfile(FRONTEND / relative, OUTPUT / relative)
    shutil.copyfile(METIS_MARK, OUTPUT / "assets" / METIS_MARK.name)
    return OUTPUT


def _page_ready(client: WebDriverClient) -> None:
    deadline = time.monotonic() + WAIT_SECONDS
    while time.monotonic() < deadline:
        state = client.execute(
            "const form = document.getElementById('greet-form');"
            "return {ready: form !== null && form.dataset.metisReady === 'true',"
            " message: document.getElementById('greet-msg')?.textContent ?? ''};"
        )
        if isinstance(state, dict) and state.get("ready") is True:
            return
        if isinstance(state, dict) and state.get("message"):
            raise BrowserRuntimeError(f"the starter reported: {state['message']}")
        time.sleep(0.1)
    raise BrowserRuntimeError(f"Rust did not bind the form within {WAIT_SECONDS} seconds")


def check(driver_url: str, capture: pathlib.Path | None, browser_name: str,
          color_scheme: str | None = None) -> str:
    """Greet `NAME` through the page and return the reply Rust wrote.

    `color_scheme` pins light or dark rendering so a capture does not follow
    the host's theme; unset, the browser uses the operating system's.
    """
    if not (OUTPUT / "index.html").is_file():
        raise BrowserRuntimeError("build the starter first: python scripts/starter.py build")
    with StaticServer(OUTPUT) as url:
        client = WebDriverClient(driver_url, 30)
        switches = (COLOR_SCHEMES[color_scheme],) if color_scheme else ()
        client.create_session(browser_name, 1000, headless=True, chromium_arguments=switches)
        try:
            client.set_window_rect(*WINDOW)
            client.navigate(url + "index.html")
            _page_ready(client)
            client.send_keys(client.find("#greet-input"), NAME)
            client.click(client.find("#greet-form button"))
            reply = client.execute("return document.getElementById('greet-msg').textContent;")
            if reply != expected_greeting(NAME):
                raise BrowserRuntimeError(f"the page shows {reply!r}, expected {expected_greeting(NAME)!r}")
            scripts = client.execute("return document.scripts.length;")
            if scripts != 1:
                raise BrowserRuntimeError(f"the page runs {scripts!r} scripts, expected the one loader")
            if color_scheme is not None:
                dark = client.execute("return matchMedia('(prefers-color-scheme: dark)').matches;")
                if dark != (color_scheme == "dark"):
                    raise BrowserRuntimeError(f"the browser did not render the {color_scheme} scheme")
            if capture is not None:
                capture.parent.mkdir(parents=True, exist_ok=True)
                capture.write_bytes(client.screenshot())
        finally:
            client.close()
    return reply


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("build", help="compile and assemble output/browser/starter")
    commands.add_parser("serve", help="serve the built page until interrupted")
    checker = commands.add_parser("check", help="greet a name through a WebDriver session")
    checker.add_argument("--driver-url", required=True, help="W3C WebDriver endpoint")
    checker.add_argument("--browser", default="MicrosoftEdge", help="WebDriver browserName")
    checker.add_argument("--capture", type=pathlib.Path, help="write a PNG capture of the page here")
    checker.add_argument("--color-scheme", choices=sorted(COLOR_SCHEMES), help="pin light or dark rendering")
    arguments = parser.parse_args(argv)
    if arguments.command == "build":
        print(build())
    elif arguments.command == "serve":
        with StaticServer(OUTPUT) as url:
            print(f"{url}index.html", flush=True)
            try:
                while True:
                    time.sleep(3600)
            except KeyboardInterrupt:
                pass
    else:
        print(check(arguments.driver_url, arguments.capture, arguments.browser, arguments.color_scheme))
    return 0


if __name__ == "__main__":
    sys.exit(main())
