"""Bounded W3C WebDriver transport and generated-asset serving."""
from __future__ import annotations

import base64
import binascii
import http.server
import json
import pathlib
import re
import struct
import threading
import urllib.error
import urllib.parse
import urllib.request
from functools import partial
from typing import Any, Dict, Mapping, Optional, Sequence


ROOT = pathlib.Path(__file__).resolve().parents[1]
MAX_SCREENSHOT_BYTES = 8 * 1024 * 1024
# Base64 expands the decoded budget by four-thirds; 256 bytes cover the JSON envelope.
MAX_SCREENSHOT_RESPONSE_BYTES = ((MAX_SCREENSHOT_BYTES + 2) // 3) * 4 + 256
MAX_TRACE_BYTES = 512 * 1024
MAX_WAIT_MILLISECONDS = 120_000
MAX_URL_BYTES = 8 * 1024
# W3C input sources are deliberately bounded so a malformed scenario cannot
# turn the driver into an unbounded action queue.
MAX_ACTION_SOURCES = 8
MAX_SOURCE_ACTIONS = 64
ELEMENT_KEY = "element-6066-11e4-a52e-4f735466cecf"


class BrowserRuntimeError(RuntimeError):
    """A browser protocol, assertion or artifact contract failed."""


def _bounded_text(value: Any, label: str, limit: int = MAX_TRACE_BYTES) -> str:
    """Convert a protocol value to bounded text for diagnostics and traces."""
    text = value if isinstance(value, str) else str(value)
    if len(text.encode("utf-8")) > limit:
        raise BrowserRuntimeError(f"{label} exceeds the {limit}-byte bound")
    return text


def _safe_path(path: pathlib.Path, *, directory: pathlib.Path) -> pathlib.Path:
    """Reject links and paths outside the repository output root."""
    resolved = path.resolve()
    root = directory.resolve()
    if not resolved.is_relative_to(root):
        raise BrowserRuntimeError(f"browser artifact path escapes {root}: {path}")
    for part in (path, *path.parents):
        if part.is_symlink():
            raise BrowserRuntimeError(f"browser artifact path is linked: {path}")
    if path.is_file() and path.stat().st_nlink != 1:
        raise BrowserRuntimeError(f"browser artifact path is hard-linked: {path}")
    return path


class _QuietHandler(http.server.SimpleHTTPRequestHandler):
    """Serve generated assets without writing request noise into the trace."""

    def log_message(self, format: str, *args: Any) -> None:  # noqa: A002 - stdlib hook name
        del format, args


class StaticServer:
    """Serve one generated browser directory on a bounded loopback port."""

    def __init__(self, directory: pathlib.Path) -> None:
        directory = _safe_path(directory, directory=ROOT / "output" / "browser")
        if not directory.is_dir():
            raise BrowserRuntimeError(f"browser serve directory is not a directory: {directory}")
        handler = partial(_QuietHandler, directory=str(directory))
        self._server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
        self._thread = threading.Thread(target=self._server.serve_forever, name="metis-browser-server", daemon=True)

    def __enter__(self) -> str:
        self._thread.start()
        port = self._server.server_address[1]
        return f"http://127.0.0.1:{port}/"

    def __exit__(self, exc_type: Any, exc: Any, traceback: Any) -> None:
        del exc_type, exc, traceback
        self._server.shutdown()
        self._server.server_close()
        self._thread.join(timeout=5)
        if self._thread.is_alive():
            raise BrowserRuntimeError("browser static server did not stop within the five-second teardown bound")


class WebDriverClient:
    """Small W3C WebDriver client with explicit request and script deadlines."""

    def __init__(self, endpoint: str, timeout_seconds: float) -> None:
        parsed = urllib.parse.urlsplit(endpoint)
        if parsed.scheme not in {"http", "https"} or not parsed.netloc:
            raise BrowserRuntimeError(f"WebDriver endpoint must be an HTTP(S) URL: {endpoint!r}")
        self._endpoint = endpoint.rstrip("/")
        self._timeout = timeout_seconds
        self.session_id: Optional[str] = None
        self.capabilities: Dict[str, Any] = {}

    def _request(
        self,
        method: str,
        path: str,
        payload: Optional[Mapping[str, Any]] = None,
        *,
        response_limit: int = MAX_TRACE_BYTES,
    ) -> Any:
        if not 1 <= response_limit <= MAX_SCREENSHOT_RESPONSE_BYTES:
            raise BrowserRuntimeError("WebDriver response limit is outside the configured bound")
        body = json.dumps(payload, separators=(",", ":")).encode("utf-8") if payload is not None else None
        request = urllib.request.Request(
            f"{self._endpoint}/{path.lstrip('/')}",
            data=body,
            headers={"Accept": "application/json", "Content-Type": "application/json"},
            method=method,
        )
        try:
            with urllib.request.urlopen(request, timeout=self._timeout) as response:
                raw = response.read(response_limit + 1)
        except urllib.error.HTTPError as error:
            raw = error.read(MAX_TRACE_BYTES + 1)
            detail = _bounded_text(raw.decode("utf-8", errors="replace"), "driver error")
            raise BrowserRuntimeError(f"WebDriver {method} {path} returned HTTP {error.code}: {detail}") from error
        except (urllib.error.URLError, TimeoutError) as error:
            raise BrowserRuntimeError(f"WebDriver {method} {path} failed: {error}") from error
        if len(raw) > response_limit:
            raise BrowserRuntimeError(f"WebDriver {method} {path} response exceeds {response_limit} bytes")
        try:
            document = json.loads(raw.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise BrowserRuntimeError(f"WebDriver {method} {path} returned invalid JSON") from error
        if not isinstance(document, dict) or "value" not in document:
            raise BrowserRuntimeError(f"WebDriver {method} {path} returned no W3C value")
        value = document["value"]
        if isinstance(value, dict) and value.get("error"):
            message = value.get("message", value["error"])
            raise BrowserRuntimeError(f"WebDriver {method} {path}: {value['error']}: {_bounded_text(message, 'driver error')}")
        return value

    def create_session(self, browser_name: str) -> None:
        """Create one session with a matrix-pinned browser name."""
        if not isinstance(browser_name, str) or not browser_name:
            raise BrowserRuntimeError("WebDriver browser name is empty")
        value = self._request(
            "POST",
            "/session",
            {"capabilities": {"alwaysMatch": {"browserName": browser_name}}},
        )
        if not isinstance(value, dict):
            raise BrowserRuntimeError("WebDriver session response is not an object")
        session_id = value.get("sessionId")
        capabilities = value.get("capabilities")
        if not isinstance(session_id, str) or not re.fullmatch(r"[A-Za-z0-9._-]+", session_id):
            raise BrowserRuntimeError("WebDriver did not return a valid session identifier")
        if not isinstance(capabilities, dict):
            raise BrowserRuntimeError("WebDriver did not return session capabilities")
        self.session_id = session_id
        self.capabilities = dict(capabilities)

    def _session_path(self, suffix: str) -> str:
        if self.session_id is None:
            raise BrowserRuntimeError("WebDriver session is not open")
        return f"/session/{self.session_id}/{suffix.lstrip('/')}"

    @staticmethod
    def _element_component(element_id: str) -> str:
        """Encode an opaque element identifier before placing it in a path."""
        if not element_id:
            raise BrowserRuntimeError("WebDriver element identifier is empty")
        return urllib.parse.quote(element_id, safe="")

    def set_timeouts(self, milliseconds: int) -> None:
        """Set the script timeout used by event-driven browser waits."""
        if not 1 <= milliseconds <= MAX_WAIT_MILLISECONDS:
            raise BrowserRuntimeError(f"script timeout must be between 1 and {MAX_WAIT_MILLISECONDS} milliseconds")
        self._request("POST", self._session_path("timeouts"), {"script": milliseconds})

    def navigate(self, url: str) -> None:
        """Navigate to a bounded HTTP(S) workbench URL."""
        _bounded_text(url, "browser URL", MAX_URL_BYTES)
        parsed = urllib.parse.urlsplit(url)
        if parsed.scheme not in {"http", "https"} or not parsed.netloc:
            raise BrowserRuntimeError(f"browser URL must be an HTTP(S) origin: {url!r}")
        self._request("POST", self._session_path("url"), {"url": url})

    def find(self, css_selector: str) -> str:
        """Find one element by CSS selector and return its opaque driver id."""
        value = self._request("POST", self._session_path("element"), {"using": "css selector", "value": css_selector})
        if not isinstance(value, dict):
            raise BrowserRuntimeError(f"element lookup returned a non-object for {css_selector!r}")
        element_id = value.get(ELEMENT_KEY) or value.get("ELEMENT")
        if not isinstance(element_id, str) or not element_id:
            raise BrowserRuntimeError(f"element lookup returned no W3C id for {css_selector!r}")
        return element_id

    def click(self, element_id: str) -> None:
        """Click one element through the browser's input dispatch."""
        encoded = self._element_component(element_id)
        self._request("POST", self._session_path(f"element/{encoded}/click"), {})

    def clear(self, element_id: str) -> None:
        """Clear an editable element through the W3C element command."""
        encoded = self._element_component(element_id)
        self._request("POST", self._session_path(f"element/{encoded}/clear"), {})

    def send_keys(self, element_id: str, value: str) -> None:
        """Send bounded text to an editable element."""
        if len(value.encode("utf-8")) > 128:
            raise BrowserRuntimeError("input value exceeds the browser trace bound")
        encoded = self._element_component(element_id)
        self._request("POST", self._session_path(f"element/{encoded}/value"), {"text": value, "value": list(value)})

    def perform_actions(self, actions: Sequence[Mapping[str, Any]]) -> None:
        """Dispatch a bounded W3C action sequence through the browser input source."""
        if isinstance(actions, (str, bytes)) or not isinstance(actions, Sequence):
            raise BrowserRuntimeError("W3C action sources must be a sequence")
        if not 1 <= len(actions) <= MAX_ACTION_SOURCES:
            raise BrowserRuntimeError(
                f"W3C action source count must be between 1 and {MAX_ACTION_SOURCES}"
            )
        sources = []
        for source in actions:
            if not isinstance(source, Mapping):
                raise BrowserRuntimeError("W3C action source is not an object")
            source_actions = source.get("actions")
            if not isinstance(source_actions, list) or not 1 <= len(source_actions) <= MAX_SOURCE_ACTIONS:
                raise BrowserRuntimeError(
                    f"W3C action source length must be between 1 and {MAX_SOURCE_ACTIONS}"
                )
            source_type = source.get("type")
            if not isinstance(source_type, str) or source_type not in {"key", "pointer", "wheel"}:
                raise BrowserRuntimeError(f"unsupported W3C action source type: {source_type!r}")
            sources.append(dict(source))
        payload = {"actions": sources}
        if len(json.dumps(payload, separators=(",", ":")).encode("utf-8")) > MAX_TRACE_BYTES:
            raise BrowserRuntimeError("W3C action sequence exceeds the trace bound")
        self._request("POST", self._session_path("actions"), payload)

    def release_actions(self) -> None:
        """Release all active W3C input sources and pointer captures."""
        self._request("DELETE", self._session_path("actions"))

    def pointer_drag(
        self,
        element_id: str,
        start: Tuple[int, int],
        end: Tuple[int, int],
        *,
        button: int = 0,
        pointer_type: str = "mouse",
        source_id: str = "metis-pointer",
    ) -> None:
        """Perform one trusted pointer drag relative to a DOM element."""
        if pointer_type not in {"mouse", "pen", "touch"}:
            raise BrowserRuntimeError(f"unsupported pointer type: {pointer_type!r}")
        if button not in range(5):
            raise BrowserRuntimeError("pointer button must be between 0 and 4")
        for coordinate in (*start, *end):
            if not isinstance(coordinate, int) or not -4096 <= coordinate <= 4096:
                raise BrowserRuntimeError("pointer coordinates must be bounded integers")
        if not element_id:
            raise BrowserRuntimeError("WebDriver element identifier is empty")
        origin = {ELEMENT_KEY: element_id}
        self.perform_actions(
            [
                {
                    "type": "pointer",
                    "id": source_id,
                    "parameters": {"pointerType": pointer_type},
                    "actions": [
                        {"type": "pointerMove", "origin": origin, "x": start[0], "y": start[1], "duration": 0},
                        {"type": "pointerDown", "button": button},
                        {"type": "pointerMove", "origin": "pointer", "x": end[0] - start[0], "y": end[1] - start[1], "duration": 0},
                        {"type": "pointerUp", "button": button},
                    ],
                }
            ]
        )

    def wheel(
        self,
        element_id: str,
        position: Tuple[int, int],
        delta: Tuple[int, int],
        *,
        source_id: str = "metis-wheel",
    ) -> None:
        """Perform one trusted wheel scroll relative to a DOM element."""
        for value in (*position, *delta):
            if not isinstance(value, int) or not -1_000_000 <= value <= 1_000_000:
                raise BrowserRuntimeError("wheel coordinates and deltas must be bounded integers")
        if not element_id:
            raise BrowserRuntimeError("WebDriver element identifier is empty")
        origin = {ELEMENT_KEY: element_id}
        self.perform_actions(
            [
                {
                    "type": "wheel",
                    "id": source_id,
                    "actions": [
                        {
                            "type": "scroll",
                            "origin": origin,
                            "x": position[0],
                            "y": position[1],
                            "deltaX": delta[0],
                            "deltaY": delta[1],
                            "duration": 0,
                        }
                    ],
                }
            ]
        )

    def execute(self, script: str, arguments: Sequence[Any] = ()) -> Any:
        """Execute a synchronous script and return its bounded JSON value."""
        value = self._request("POST", self._session_path("execute/sync"), {"script": script, "args": list(arguments)})
        encoded = json.dumps(value, separators=(",", ":"))
        if len(encoded.encode("utf-8")) > MAX_TRACE_BYTES:
            raise BrowserRuntimeError("browser script result exceeds the trace bound")
        return value

    def execute_async(self, script: str, arguments: Sequence[Any] = ()) -> Any:
        """Execute an event-driven asynchronous browser script."""
        return self.execute_endpoint("execute/async", script, arguments)

    def execute_endpoint(self, endpoint: str, script: str, arguments: Sequence[Any] = ()) -> Any:
        """Execute a script at an explicit W3C endpoint."""
        value = self._request("POST", self._session_path(endpoint), {"script": script, "args": list(arguments)})
        encoded = json.dumps(value, separators=(",", ":"))
        if len(encoded.encode("utf-8")) > MAX_TRACE_BYTES:
            raise BrowserRuntimeError("browser script result exceeds the trace bound")
        return value

    def screenshot(self) -> bytes:
        """Decode one PNG screenshot and enforce its byte budget."""
        value = self._request(
            "GET",
            self._session_path("screenshot"),
            response_limit=MAX_SCREENSHOT_RESPONSE_BYTES,
        )
        return _decode_screenshot(value)

    def element_screenshot(self, element_id: str) -> bytes:
        """Capture one element's PNG through the W3C element endpoint."""
        encoded = self._element_component(element_id)
        value = self._request(
            "GET",
            self._session_path(f"element/{encoded}/screenshot"),
            response_limit=MAX_SCREENSHOT_RESPONSE_BYTES,
        )
        return _decode_screenshot(value)

    def close(self) -> None:
        """Close the session exactly once when it exists."""
        if self.session_id is None:
            return
        session = self.session_id
        self.session_id = None
        self._request("DELETE", f"/session/{session}")


def _decode_screenshot(value: Any) -> bytes:
    """Decode and validate a W3C base64 PNG response."""
    if not isinstance(value, str):
        raise BrowserRuntimeError("WebDriver screenshot response is not base64 text")
    try:
        content = base64.b64decode(value, validate=True)
    except (ValueError, binascii.Error) as error:
        raise BrowserRuntimeError("WebDriver screenshot is not valid base64") from error
    if len(content) > MAX_SCREENSHOT_BYTES:
        raise BrowserRuntimeError("browser screenshot exceeds the 8 MiB budget")
    if content[:8] != b"\x89PNG\r\n\x1a\n" or len(content) < 24 or content[12:16] != b"IHDR":
        raise BrowserRuntimeError("browser screenshot is not a PNG")
    width, height = struct.unpack(">II", content[16:24])
    if not (0 < width <= 4096 and 0 < height <= 4096):
        raise BrowserRuntimeError("browser screenshot dimensions exceed the 4096-pixel bound")
    return content
