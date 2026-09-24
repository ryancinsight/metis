"""Check the Metis starter application in a browser.

Runs `metis serve` on the starter's manifest, which builds the page, and
drives it through a W3C WebDriver endpoint: it greets a name, verifies the
reply Rust wrote, and can capture the page. Building and serving belong to
the `metis` command; this script is only the browser check.
"""
from __future__ import annotations

import argparse
import contextlib
import json
import pathlib
import queue
import subprocess
import sys
import threading
import time
from typing import Iterator

import browser
from browser_protocol import ROOT, BrowserRuntimeError, WebDriverClient
from cargo_overlay import isolated_cargo

STARTER = ROOT / "crates" / "metis-starter"
MANIFEST = STARTER / "metis.json"
# The template's window size, so a capture matches the Tauri starter's frame.
WINDOW = (800, 600)
WAIT_SECONDS = 10
# A cold release build of the module and its dependencies fits in five minutes
# on this host; the hosted runner does not run this check.
SERVE_READY_SECONDS = 300
NAME = "Metis"
# Chromium's preferred-color-scheme setting: 1 selects light and 0 dark,
# measured against the page's prefers-color-scheme media query.
COLOR_SCHEMES = {"light": "--blink-settings=preferredColorScheme=1",
                 "dark": "--blink-settings=preferredColorScheme=0"}


def expected_greeting(name: str) -> str:
    """The reply `metis_starter::greet` returns; its doctest pins the wording."""
    return f"Hello, {name}! You've been greeted from Rust!"


def metis_command() -> pathlib.Path:
    """Build the `metis` command against the committed lock and return its path."""
    browser.cargo(["build", "--locked", "-p", "metis-cli"])
    metadata = browser.cargo(
        ["metadata", "--no-deps", "--format-version", "1", "--locked"],
        timeout=30, capture_output=True, text=True,
    )
    target = pathlib.Path(json.loads(metadata.stdout)["target_directory"])
    command = target / "debug" / ("metis.exe" if sys.platform == "win32" else "metis")
    if not command.is_file():
        raise BrowserRuntimeError(f"Cargo did not produce the metis command: {command}")
    return command


@contextlib.contextmanager
def served(command: pathlib.Path) -> Iterator[str]:
    """Run `metis serve` on an ephemeral port; yield the address it prints."""
    with isolated_cargo(ROOT) as (directory, environment):
        # The gate's pinned CLI, whose version the locked crate requires.
        environment["WASM_BINDGEN"] = browser.wasm_bindgen()
        process = subprocess.Popen(
            [str(command), "serve", str(MANIFEST), "--port", "0"],
            cwd=directory, env=environment, stdout=subprocess.PIPE, text=True, encoding="utf-8",
        )
        lines: queue.Queue[str | None] = queue.Queue()

        def forward() -> None:
            for line in process.stdout:
                lines.put(line)
            lines.put(None)

        threading.Thread(target=forward, daemon=True).start()
        try:
            deadline = time.monotonic() + SERVE_READY_SECONDS
            while True:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise BrowserRuntimeError(f"metis serve printed no address in {SERVE_READY_SECONDS} s")
                try:
                    line = lines.get(timeout=remaining)
                except queue.Empty:
                    continue
                if line is None:
                    raise BrowserRuntimeError(f"metis serve exited with status {process.wait(timeout=5)}")
                if "http://127.0.0.1:" in line:
                    yield line[line.index("http://"):].strip()
                    return
        finally:
            process.terminate()
            process.wait(timeout=5)


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
    """Greet `NAME` through the served page and return the reply Rust wrote.

    `color_scheme` pins light or dark rendering so a capture does not follow
    the host's theme; unset, the browser uses the operating system's.
    """
    with served(metis_command()) as url:
        client = WebDriverClient(driver_url, 30)
        switches = (COLOR_SCHEMES[color_scheme],) if color_scheme else ()
        client.create_session(browser_name, 1000, headless=True, chromium_arguments=switches)
        try:
            client.set_window_rect(*WINDOW)
            client.navigate(url)
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
    parser.add_argument("--driver-url", required=True, help="W3C WebDriver endpoint")
    parser.add_argument("--browser", default="MicrosoftEdge", help="WebDriver browserName")
    parser.add_argument("--capture", type=pathlib.Path, help="write a PNG capture of the page here")
    parser.add_argument("--color-scheme", choices=sorted(COLOR_SCHEMES), help="pin light or dark rendering")
    arguments = parser.parse_args(argv)
    print(check(arguments.driver_url, arguments.capture, arguments.browser, arguments.color_scheme))
    return 0


if __name__ == "__main__":
    sys.exit(main())
