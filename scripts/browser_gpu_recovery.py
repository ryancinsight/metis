"""Build and verify real WebGPU canvas recovery through W3C WebDriver."""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import sys
from typing import Any, Mapping

import browser
from browser_canvas import _element_screenshot, ensure_canvas_visible
from browser_gpu_trace import (
    DESTROY_AND_WAIT,
    INSTALL_GPU_TRACE,
    PAGE,
    READ_PIXELS,
    READ_SCREENSHOT_PIXELS,
    WAIT_FOR_LOADER,
    WAIT_FOR_STATUS,
)
from browser_protocol import MAX_TRACE_BYTES, ROOT, BrowserRuntimeError, StaticServer, WebDriverClient
from browser_runtime import _write_trace
from browser_trace import BrowserEngine, Trace


OUTPUT = ROOT / "output" / "browser" / "gpu-recovery"
CANVAS_ID = "recovery"
CANVAS_SIZE = 32
WAIT_MILLISECONDS = 10_000
MAX_GPU_DEVICES = 4
MAX_GPU_EVENTS = 64
SOURCE_PATHS = (
    "Cargo.lock",
    "crates/metis-web/Cargo.toml",
    "crates/metis-web/examples/canvas_recovery.rs",
    "crates/metis-web/examples/canvas_recovery/browser.rs",
    "crates/metis-web/src/canvas/surface.rs",
    "crates/metis-web/src/canvas/frame.rs",
    "scripts/browser.py",
    "scripts/browser_canvas.py",
    "scripts/browser_gpu_recovery.py",
    "scripts/browser_gpu_trace.py",
    "scripts/browser_protocol.py",
)

def _revision() -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, check=True, capture_output=True,
        text=True, timeout=30,
    )
    revision = result.stdout.strip()
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise BrowserRuntimeError(f"git returned an invalid revision: {revision!r}")
    return revision


def _digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _source_basis() -> dict[str, str]:
    basis = {}
    for relative in SOURCE_PATHS:
        path = ROOT / relative
        if not path.is_file():
            raise BrowserRuntimeError(f"recovery input is missing: {relative}")
        basis[relative] = _digest(path)
    return basis


def _dirty() -> bool:
    result = subprocess.run(
        ["git", "status", "--porcelain", "--untracked-files=all"], cwd=ROOT,
        check=True, capture_output=True, text=True, timeout=30,
    )
    return bool(result.stdout)


def _prepare_package() -> pathlib.Path:
    OUTPUT.mkdir(parents=True, exist_ok=True)
    for name in ("trace.json", "initial.png", "recreated.png"):
        (OUTPUT / name).unlink(missing_ok=True)
    browser.cargo([
        "build", "--locked", "-p", "metis-web", "--example", "canvas_recovery",
        "--target", "wasm32-unknown-unknown", "--release",
    ])
    metadata = browser.cargo(
        ["metadata", "--no-deps", "--format-version", "1", "--locked"],
        timeout=30, capture_output=True, text=True,
    )
    target_directory = pathlib.Path(json.loads(metadata.stdout)["target_directory"])
    module = target_directory / "wasm32-unknown-unknown" / "release" / "examples" / "canvas_recovery.wasm"
    if not module.is_file():
        raise BrowserRuntimeError(f"Cargo did not produce the recovery module: {module}")
    subprocess.run(
        [browser.wasm_bindgen(), str(module), "--target", "web", "--out-dir", str(OUTPUT)],
        cwd=ROOT, check=True, timeout=300,
    )
    (OUTPUT / "index.html").write_text(PAGE, encoding="utf-8")
    return module


def _require_ok(value: Any, operation: str) -> Mapping[str, Any]:
    if not isinstance(value, dict) or value.get("ok") is not True:
        detail = value.get("error") if isinstance(value, dict) else value
        raise BrowserRuntimeError(f"{operation} failed: {detail!r}")
    return value


def _wait_status(client: WebDriverClient, expected: int) -> int:
    value = _require_ok(
        client.execute_async(WAIT_FOR_STATUS, [expected, WAIT_MILLISECONDS]),
        f"waiting for recovery status {expected}",
    )
    if value.get("status") != expected:
        raise BrowserRuntimeError(f"recovery status is {value.get('status')!r}, expected {expected}")
    return expected


def _listener_count(client: WebDriverClient) -> int:
    value = client.execute("return Number(window.recovery.canvas_listeners());")
    if type(value) is not int or value < 0 or value > MAX_GPU_EVENTS:
        raise BrowserRuntimeError(f"recovery listener count is invalid: {value!r}")
    return value


def _pixel_observation(value: Any, left: tuple[int, ...], right: tuple[int, ...]) -> dict[str, Any]:
    value = _require_ok(value, "canvas pixel readback")
    if value.get("width") != CANVAS_SIZE or value.get("height") != CANVAS_SIZE:
        raise BrowserRuntimeError(f"recovery canvas dimensions are invalid: {value!r}")
    pixels = value.get("pixels")
    expected_length = CANVAS_SIZE * CANVAS_SIZE * 4
    if not isinstance(pixels, list) or len(pixels) != expected_length:
        raise BrowserRuntimeError("recovery canvas returned an invalid RGBA buffer")
    raw = bytearray()
    for index, component in enumerate(pixels):
        if type(component) is not int or not 0 <= component <= 255:
            raise BrowserRuntimeError(f"RGBA component {index} is invalid: {component!r}")
        raw.append(component)
    for pixel_index in range(CANVAS_SIZE * CANVAS_SIZE):
        offset = pixel_index * 4
        expected = left if pixel_index % CANVAS_SIZE < CANVAS_SIZE // 2 else right
        actual = tuple(raw[offset:offset + 4])
        if actual != expected:
            x = pixel_index % CANVAS_SIZE
            y = pixel_index // CANVAS_SIZE
            raise BrowserRuntimeError(f"pixel ({x}, {y}) is {actual}, expected {expected}")
    return {
        "width": CANVAS_SIZE,
        "height": CANVAS_SIZE,
        "rgba_sha256": hashlib.sha256(raw).hexdigest(),
        "left_rgba": list(left),
        "right_rgba": list(right),
    }


def _transparent_readback(value: Any) -> bool:
    if not isinstance(value, dict) or value.get("ok") is not True:
        return False
    pixels = value.get("pixels")
    return (
        value.get("width") == CANVAS_SIZE
        and value.get("height") == CANVAS_SIZE
        and isinstance(pixels, list)
        and len(pixels) == CANVAS_SIZE * CANVAS_SIZE * 4
        and all(component == 0 for component in pixels)
    )


def _observe_pixels(
    client: WebDriverClient,
    label: str,
    left: tuple[int, ...],
    right: tuple[int, ...],
) -> dict[str, Any]:
    screenshot = client.execute_async(
        READ_SCREENSHOT_PIXELS, [f"{label}.png", WAIT_MILLISECONDS]
    )
    observation = _pixel_observation(screenshot, left, right)
    observation["readback"] = "element-screenshot"
    draw_image = client.execute_async(READ_PIXELS, [CANVAS_ID, WAIT_MILLISECONDS])
    if _transparent_readback(draw_image):
        observation["draw_image"] = {
            "available": False,
            "reason": "WebGPU swapchain was transparent after presentation",
        }
    else:
        observation["draw_image"] = {
            "available": True,
            **_pixel_observation(draw_image, left, right),
        }
    return observation


def _capture(client: WebDriverClient, trace: Trace, label: str) -> None:
    ensure_canvas_visible(client, CANVAS_ID)
    _element_screenshot(client, trace, OUTPUT, label, client.find(f"#{CANVAS_ID}"))


def _validate_gpu_trace(value: Any) -> Mapping[str, Any]:
    if not isinstance(value, dict):
        raise BrowserRuntimeError("WebGPU trace is not an object")
    devices = value.get("devices")
    configurations = value.get("configurations")
    uploads = value.get("uploads")
    errors = value.get("errors")
    if value.get("overflow") is not False:
        raise BrowserRuntimeError("WebGPU trace exceeded its event bound")
    if not isinstance(devices, list) or len(devices) != 2:
        raise BrowserRuntimeError(f"recovery created {len(devices) if isinstance(devices, list) else 'invalid'} devices")
    for name, records in (("configurations", configurations), ("uploads", uploads)):
        if (
            not isinstance(records, list)
            or not all(isinstance(item, dict) for item in records)
            or [item.get("device_id") for item in records] != [1, 2]
        ):
            raise BrowserRuntimeError(f"recovery {name} must cover devices 1 then 2 exactly once")
    if not isinstance(errors, list) or errors:
        raise BrowserRuntimeError(f"WebGPU recovery recorded errors: {errors!r}")
    if not all(isinstance(item, dict) for item in devices) or [item.get("id") for item in devices] != [1, 2]:
        raise BrowserRuntimeError("recovery device identities must be 1 then 2")
    if devices[1].get("lost") is not None:
        raise BrowserRuntimeError("replacement device was lost")
    if any(item.get("uncaptured_errors") != [] for item in devices):
        raise BrowserRuntimeError("device recorded uncaptured errors")
    first_loss = devices[0].get("lost")
    if not isinstance(first_loss, dict) or first_loss.get("reason") != "destroyed":
        raise BrowserRuntimeError(f"initial device loss was not observed as destroyed: {first_loss!r}")
    return value


def run(webdriver: str, browser_name: str, timeout_seconds: float) -> dict[str, Any]:
    """Build the example and execute its bounded loss/recovery lifecycle."""
    if not 0 < timeout_seconds <= 120:
        raise BrowserRuntimeError("timeout-seconds must be greater than zero and at most 120")
    OUTPUT.mkdir(parents=True, exist_ok=True)
    (OUTPUT / "trace.json").unlink(missing_ok=True)
    source_basis = _source_basis()
    module = _prepare_package()
    revision = _revision()
    client = WebDriverClient(webdriver, timeout_seconds)
    trace: Trace | None = None
    stage = "session"
    try:
        with StaticServer(OUTPUT) as origin:
            client.create_session(browser_name, 1000)
            client.set_timeouts(max(WAIT_MILLISECONDS + 2_000, int(timeout_seconds * 1000)))
            client.set_window_rect(640, 480)
            client.navigate(origin)
            trace = Trace(BrowserEngine.CHROMIUM, origin, "disconnected", revision, client.capabilities)
            device_pixel_ratio = client.execute("return window.devicePixelRatio;")
            if device_pixel_ratio != 1:
                raise BrowserRuntimeError(
                    f"exact screenshot oracle requires devicePixelRatio 1, found {device_pixel_ratio!r}"
                )
            _require_ok(client.execute_async(WAIT_FOR_LOADER, [WAIT_MILLISECONDS]), "WASM loader")
            _require_ok(
                client.execute(INSTALL_GPU_TRACE, [MAX_GPU_DEVICES, MAX_GPU_EVENTS]),
                "WebGPU instrumentation",
            )
            stage = "initial-presentation"
            client.execute("window.recovery.canvas_start();")
            _wait_status(client, 2)
            initial_listeners = _listener_count(client)
            if initial_listeners <= 0:
                raise BrowserRuntimeError("initial recovery surface installed no listeners")
            canvas_identity = client.execute(
                "window.__recoveryCanvas = document.getElementById(arguments[0]); "
                "return window.__recoveryCanvas instanceof HTMLCanvasElement;", [CANVAS_ID],
            )
            if canvas_identity is not True:
                raise BrowserRuntimeError("recovery canvas is missing")
            _capture(client, trace, "initial")
            initial_pixels = _observe_pixels(
                client, "initial", (255, 0, 0, 255), (0, 255, 0, 255),
            )

            stage = "device-loss"
            loss = dict(_require_ok(
                client.execute_async(DESTROY_AND_WAIT, [WAIT_MILLISECONDS]), "device loss",
            ))
            if loss.get("reason") != "destroyed":
                raise BrowserRuntimeError(f"device.lost returned unexpected reason: {loss.get('reason')!r}")
            stage = "post-loss-redraw"
            client.execute("window.recovery.canvas_redraw();")
            redraw_after_loss = client.execute("return Number(window.recovery.canvas_status());")
            redraw_diagnostic = client.execute(
                "const element = document.getElementById('recovery-error'); "
                "return element ? (element.textContent || '').trim().slice(0, 512) : '';"
            )
            if (
                redraw_after_loss != 255
                or not isinstance(redraw_diagnostic, str)
                or not redraw_diagnostic.startswith("Other:")
                or "lost" not in redraw_diagnostic.casefold()
            ):
                raise BrowserRuntimeError(
                    "post-loss redraw did not surface a typed failure: "
                    f"status={redraw_after_loss!r}, diagnostic={redraw_diagnostic!r}"
                )

            stage = "recreation"
            client.execute("window.recovery.canvas_recreate();")
            _wait_status(client, 3)
            recovered_listeners = _listener_count(client)
            if recovered_listeners != initial_listeners:
                raise BrowserRuntimeError(
                    f"listener count changed across recovery: {initial_listeners} -> {recovered_listeners}"
                )
            same_canvas = client.execute(
                "return window.__recoveryCanvas === document.getElementById(arguments[0]);", [CANVAS_ID],
            )
            if same_canvas is not True:
                raise BrowserRuntimeError("recovery replaced the canvas element")
            new_device = client.execute(
                "const devices = window.__gpuRecoveryDevices || []; "
                "return devices.length === 2 && devices[0] !== devices[1];"
            )
            if new_device is not True:
                raise BrowserRuntimeError("recovery did not create one distinct replacement device")
            _capture(client, trace, "recreated")
            recovered_pixels = _observe_pixels(
                client, "recreated", (0, 0, 255, 255), (255, 255, 255, 255),
            )

            stage = "cleanup"
            client.execute("window.recovery.canvas_stop();")
            stopped_status = client.execute("return Number(window.recovery.canvas_status());")
            stopped_listeners = _listener_count(client)
            if stopped_status != 4 or stopped_listeners != 0:
                raise BrowserRuntimeError(
                    f"stop cleanup is invalid: status={stopped_status!r}, listeners={stopped_listeners!r}"
                )
            gpu = _validate_gpu_trace(client.execute("return window.__gpuRecoveryTrace;"))
            if _source_basis() != source_basis:
                raise BrowserRuntimeError("recovery inputs changed while the browser run was active")
            document = {
                "schema": 1,
                "status": "passed",
                "revision": revision,
                "dirty": _dirty(),
                "basis": {
                    "sources": source_basis,
                    "wasm_module_sha256": _digest(module),
                    "wasm_bindgen_sha256": _digest(OUTPUT / "canvas_recovery_bg.wasm"),
                    "loader_sha256": _digest(OUTPUT / "canvas_recovery.js"),
                },
                "url": origin,
                "browser_name": browser_name,
                "device_pixel_ratio": device_pixel_ratio,
                "capabilities": dict(client.capabilities),
                "canvas": {
                    "id": CANVAS_ID,
                    "same_element": True,
                    "listeners": {"initial": initial_listeners, "recreated": recovered_listeners, "stopped": 0},
                    "pixels": {"initial": initial_pixels, "recreated": recovered_pixels},
                    "screenshots": trace.screenshots,
                },
                "gpu": gpu,
                "loss": loss,
                "post_loss_redraw_status": redraw_after_loss,
                "post_loss_redraw_diagnostic": redraw_diagnostic,
                "cleanup": {"status": stopped_status, "listeners": stopped_listeners},
            }
            return document
    except BrowserRuntimeError as error:
        browser_state: dict[str, Any] = {}
        if client.session_id is not None:
            try:
                browser_state = client.execute("""
const recovery = window.recovery;
const element = document.getElementById('recovery-error');
return {
  status: recovery ? Number(recovery.canvas_status()) : null,
  listeners: recovery ? Number(recovery.canvas_listeners()) : null,
  diagnostic: element ? (element.textContent || '').trim().slice(0, 512) : '',
  gpu: window.__gpuRecoveryTrace || null
};
""")
            except BrowserRuntimeError as diagnostic_error:
                browser_state = {"capture_error": str(diagnostic_error)[:1024]}
        failure = {
            "schema": 1,
            "status": "failed",
            "stage": stage,
            "error": str(error)[:4096],
            "revision": revision,
            "dirty": _dirty(),
            "basis": {
                "sources": source_basis,
                "wasm_module_sha256": _digest(module),
                "wasm_bindgen_sha256": _digest(OUTPUT / "canvas_recovery_bg.wasm"),
                "loader_sha256": _digest(OUTPUT / "canvas_recovery.js"),
            },
            "capabilities": dict(client.capabilities),
            "screenshots": trace.screenshots if trace is not None else [],
            "browser": browser_state,
        }
        _write_trace(OUTPUT / "trace.json", failure)
        raise
    finally:
        client.close()


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--webdriver", default="http://127.0.0.1:9515")
    parser.add_argument("--browser-name", default="MicrosoftEdge")
    parser.add_argument("--timeout-seconds", type=float, default=30.0)
    return parser.parse_args()


def main() -> int:
    arguments = _arguments()
    try:
        document = run(arguments.webdriver, arguments.browser_name, arguments.timeout_seconds)
    except (BrowserRuntimeError, OSError, subprocess.SubprocessError, ValueError) as error:
        trace_path = OUTPUT / "trace.json"
        trace_error = None
        if not trace_path.is_file():
            try:
                _write_trace(trace_path, {"schema": 1, "status": "failed", "error": str(error)[:4096]})
            except (BrowserRuntimeError, OSError) as write_error:
                trace_error = f"; failure trace could not be written: {write_error}"
        print(f"browser GPU recovery failed: {error}{trace_error or ''}", file=sys.stderr)
        return 1
    document["cleanup"]["session_closed"] = True
    _write_trace(OUTPUT / "trace.json", document)
    encoded = json.dumps(document, sort_keys=True)
    if len(encoded.encode("utf-8")) > MAX_TRACE_BYTES:
        raise BrowserRuntimeError("browser GPU recovery result exceeds the trace bound")
    print(f"browser GPU recovery passed: {OUTPUT / 'trace.json'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
