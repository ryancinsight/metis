"""Wait for a bounded W3C WebDriver status endpoint."""
from __future__ import annotations

import argparse
import json
import time
import urllib.error
import urllib.parse
import urllib.request

from browser_protocol import MAX_TRACE_BYTES, BrowserRuntimeError


def wait_until_ready(webdriver: str, timeout_seconds: float) -> None:
    """Wait until one WebDriver endpoint reports that it can accept sessions."""
    parsed = urllib.parse.urlsplit(webdriver)
    if parsed.scheme not in {"http", "https"} or not parsed.netloc:
        raise BrowserRuntimeError(f"WebDriver endpoint must be an HTTP(S) URL: {webdriver!r}")
    if not 0 < timeout_seconds <= 120:
        raise BrowserRuntimeError("WebDriver health timeout must be in (0, 120] seconds")

    endpoint = webdriver.rstrip("/") + "/status"
    deadline = time.monotonic() + timeout_seconds
    last_error = "no response"
    while time.monotonic() < deadline:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        try:
            with urllib.request.urlopen(endpoint, timeout=min(1.0, remaining)) as response:
                raw = response.read(MAX_TRACE_BYTES + 1)
                if len(raw) > MAX_TRACE_BYTES:
                    last_error = "driver status response exceeds the trace bound"
                elif response.status != 200:
                    last_error = f"HTTP {response.status}"
                else:
                    document = json.loads(raw.decode("utf-8"))
                    value = document.get("value") if isinstance(document, dict) else None
                    if isinstance(value, dict) and value.get("ready") is True:
                        return
                    last_error = "driver status is not ready"
        except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
            last_error = str(error)
        remaining = deadline - time.monotonic()
        if remaining > 0:
            time.sleep(min(0.1, remaining))
    raise BrowserRuntimeError(
        f"WebDriver did not become ready within {timeout_seconds:g} seconds: {last_error}"
    )


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--webdriver", required=True)
    parser.add_argument("--timeout-seconds", required=True, type=float)
    return parser.parse_args()


def main() -> int:
    arguments = _arguments()
    try:
        wait_until_ready(arguments.webdriver, arguments.timeout_seconds)
    except BrowserRuntimeError as error:
        raise SystemExit(str(error)) from error
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
