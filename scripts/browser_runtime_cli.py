"""Run the Metis browser workbench through a W3C WebDriver endpoint."""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import subprocess
from typing import Any, Dict, Optional

from browser_protocol import (
    MAX_WAIT_MILLISECONDS,
    ROOT,
    BrowserRuntimeError,
    StaticServer,
    WebDriverClient,
    parse_device_scale,
)
from browser_runtime import (
    BRIDGE_MODES,
    MAX_LIFECYCLE_CYCLES,
    _validate_bridge_url,
    _write_trace,
    run_scenario,
)
from browser_trace import BrowserEngine, Trace, UNSUPPORTED_NATIVE_OPERATIONS


def _revision() -> str:
    """Read the exact source revision bound to the trace."""
    try:
        result = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, check=True, capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.SubprocessError) as error:
        raise BrowserRuntimeError(f"cannot read the Metis revision: {error}") from error
    revision = result.stdout.strip()
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise BrowserRuntimeError(f"git returned an invalid revision: {revision!r}")
    return revision


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", required=True, choices=[engine.value for engine in BrowserEngine])
    parser.add_argument("--browser-name", help="W3C browserName override within the selected engine family")
    parser.add_argument("--device-scale", help="requested browser device scale between 0.5 and 4, in decimal form")
    parser.add_argument("--scenario", choices=("workbench", "canvas", "fragment"), default="workbench")
    parser.add_argument("--driver-url", help="W3C WebDriver endpoint; defaults to METIS_WEBDRIVER_<ENGINE>_URL")
    parser.add_argument("--url", help="already-running browser workbench URL")
    parser.add_argument("--serve-dir", type=pathlib.Path, help="serve one generated output/browser directory on loopback")
    parser.add_argument("--canvas-id", action="append", default=[], help="canvas DOM id for the format-neutral trusted-input scenario; repeat per canvas")
    parser.add_argument("--canvas-attribute", action="append", default=[], help="consumer-selected data-* attribute to capture on each canvas; repeat per attribute")
    parser.add_argument(
        "--keyboard-trace",
        nargs="?",
        const="navigation",
        choices=("navigation", "cine-rate"),
        metavar="{navigation,cine-rate}",
        help="include focused keyboard evidence; default profile is ArrowDown navigation",
    )
    parser.add_argument("--consumer-revision", help="40-hex revision of the application consuming the format-neutral canvas seam")
    parser.add_argument("--bridge", choices=BRIDGE_MODES, default="disconnected")
    parser.add_argument("--cancel", action="store_true", help="submit a delayed authorized request, stop, remount and check stale-response disposal")
    parser.add_argument("--cancel-grace-ms", type=int, default=4_000)
    parser.add_argument("--browser-heap-sample", action="store_true", help="record bounded performance.memory JavaScript-heap observations when exposed")
    parser.add_argument("--browser-memory-sample", action="store_true", help="record bounded measureUserAgentSpecificMemory observations when exposed")
    parser.add_argument("--accessibility-probe", action="store_true", help="record bounded browser media, focus-order and zoom geometry evidence")
    parser.add_argument("--asset-probe", action="store_true", help="decode the shipped same-origin SVG and PNG marks and record intrinsic dimensions")
    parser.add_argument("--text-geometry-probe", action="store_true", help="record bounded Range/grapheme layout geometry from the text specimen")
    parser.add_argument("--require-reduced-motion", action="store_true", help="fail unless prefers-reduced-motion: reduce is active (requires --accessibility-probe)")
    parser.add_argument("--require-forced-colors", action="store_true", help="fail unless forced-colors: active is active (requires --accessibility-probe)")
    parser.add_argument("--lifecycle-cycles", type=int, default=1, help=f"repeat the workbench stop/remount lifecycle between 1 and {MAX_LIFECYCLE_CYCLES} times")
    parser.add_argument("--timeout-seconds", type=float, default=30.0)
    parser.add_argument("--output", type=pathlib.Path, help="trace path; defaults to output/browser/runtime/<engine>-<scenario>.json")
    return parser.parse_args()


def main() -> int:
    """Run one selected engine and return a process status."""
    arguments = _arguments()
    engine = BrowserEngine.parse(arguments.engine)
    output = (arguments.output or ROOT / "output" / "browser" / "runtime" / f"{engine.value}-{arguments.scenario}.json").resolve()
    trace: Optional[Trace] = None
    client: Optional[WebDriverClient] = None
    try:
        if arguments.timeout_seconds <= 0 or arguments.timeout_seconds > 120:
            raise BrowserRuntimeError("timeout-seconds must be greater than zero and at most 120")
        timeout_ms = int(arguments.timeout_seconds * 1000)
        browser_name = engine.resolve_webdriver_name(arguments.browser_name)
        device_scale_milli = (
            parse_device_scale(arguments.device_scale)
            if arguments.device_scale is not None
            else None
        )
        if not 1 <= arguments.cancel_grace_ms <= MAX_WAIT_MILLISECONDS:
            raise BrowserRuntimeError(f"cancel-grace-ms must be between 1 and {MAX_WAIT_MILLISECONDS}")
        if type(arguments.lifecycle_cycles) is not int or not 1 <= arguments.lifecycle_cycles <= MAX_LIFECYCLE_CYCLES:
            raise BrowserRuntimeError(
                f"lifecycle-cycles must be between 1 and {MAX_LIFECYCLE_CYCLES}"
            )
        if arguments.cancel and arguments.bridge != "authorized":
            raise BrowserRuntimeError("--cancel requires --bridge authorized")
        if arguments.require_reduced_motion and not arguments.accessibility_probe:
            raise BrowserRuntimeError("--require-reduced-motion requires --accessibility-probe")
        if arguments.require_forced_colors and not arguments.accessibility_probe:
            raise BrowserRuntimeError("--require-forced-colors requires --accessibility-probe")
        if arguments.accessibility_probe and arguments.scenario != "workbench":
            raise BrowserRuntimeError("--accessibility-probe requires --scenario workbench")
        if arguments.asset_probe and arguments.scenario != "workbench":
            raise BrowserRuntimeError("--asset-probe requires --scenario workbench")
        if arguments.text_geometry_probe and arguments.scenario != "workbench":
            raise BrowserRuntimeError("--text-geometry-probe requires --scenario workbench")
        run_canvas_scenario = None
        run_fragment_scenario = None
        consumer_revision = None
        keyboard_trace = None
        if arguments.scenario == "fragment":
            from browser_fragment import run_fragment_scenario

            if arguments.bridge != "disconnected":
                raise BrowserRuntimeError("fragment scenarios use the HTTP boundary, not the workbench bridge")
            if arguments.cancel:
                raise BrowserRuntimeError("--cancel is only valid for the workbench scenario")
            if arguments.lifecycle_cycles != 1:
                raise BrowserRuntimeError("--lifecycle-cycles requires --scenario workbench")
            if arguments.canvas_id:
                raise BrowserRuntimeError("--canvas-id requires --scenario canvas")
            if arguments.consumer_revision is not None:
                raise BrowserRuntimeError("--consumer-revision requires --scenario canvas")
            if arguments.canvas_attribute:
                raise BrowserRuntimeError("--canvas-attribute requires --scenario canvas")
            if arguments.keyboard_trace:
                raise BrowserRuntimeError("--keyboard-trace requires --scenario canvas")
        elif arguments.scenario == "canvas":
            from browser_canvas import (
                KeyboardTraceKind,
                run_canvas_scenario,
                validate_canvas_attributes,
                validate_canvas_ids,
                validate_consumer_revision,
            )

            if arguments.bridge != "disconnected":
                raise BrowserRuntimeError("canvas scenarios do not use the workbench bridge")
            if arguments.cancel:
                raise BrowserRuntimeError("--cancel is only valid for the workbench scenario")
            if arguments.lifecycle_cycles != 1:
                raise BrowserRuntimeError("--lifecycle-cycles requires --scenario workbench")
            canvas_ids = validate_canvas_ids(arguments.canvas_id)
            canvas_attributes = validate_canvas_attributes(arguments.canvas_attribute)
            consumer_revision = validate_consumer_revision(arguments.consumer_revision)
            keyboard_trace = (
                KeyboardTraceKind.parse(arguments.keyboard_trace)
                if arguments.keyboard_trace is not None
                else None
            )
        elif arguments.keyboard_trace:
            raise BrowserRuntimeError("--keyboard-trace requires --scenario canvas")
        elif arguments.canvas_id:
            raise BrowserRuntimeError("--canvas-id requires --scenario canvas")
        elif arguments.consumer_revision is not None:
            raise BrowserRuntimeError("--consumer-revision requires --scenario canvas")
        elif arguments.canvas_attribute:
            raise BrowserRuntimeError("--canvas-attribute requires --scenario canvas")
        driver_url = arguments.driver_url or os.environ.get(f"METIS_WEBDRIVER_{engine.value.upper()}_URL")
        if not driver_url:
            raise BrowserRuntimeError(f"set --driver-url or METIS_WEBDRIVER_{engine.value.upper()}_URL")
        if arguments.url and arguments.serve_dir:
            raise BrowserRuntimeError("--url and --serve-dir are mutually exclusive")
        if not arguments.url and not arguments.serve_dir:
            raise BrowserRuntimeError("one of --url or --serve-dir is required")
        if arguments.bridge == "authorized" and arguments.serve_dir:
            raise BrowserRuntimeError("authorized runs require --url with host session configuration")
        if arguments.scenario == "fragment" and arguments.serve_dir:
            raise BrowserRuntimeError("fragment scenarios require --url for the HTTP service origin")
        revision = _revision()
        server = StaticServer(arguments.serve_dir) if arguments.serve_dir else None
        if server is not None:
            with server as origin:
                url = origin
                client = WebDriverClient(driver_url, arguments.timeout_seconds)
                if arguments.scenario == "canvas":
                    trace = run_canvas_scenario(
                        client,
                        engine,
                        url,
                        revision,
                        output.parent / "screenshots" / engine.value / "canvas",
                        timeout_ms,
                        canvas_ids,
                        consumer_revision,
                        canvas_attributes,
                        browser_heap=arguments.browser_heap_sample,
                        browser_memory=arguments.browser_memory_sample,
                        keyboard_trace=keyboard_trace,
                        browser_name=browser_name,
                        device_scale_milli=device_scale_milli,
                    )
                elif arguments.scenario == "fragment":
                    raise BrowserRuntimeError("fragment scenarios require --url for the HTTP service origin")
                else:
                    trace = run_scenario(
                        client,
                        engine,
                        url,
                        arguments.bridge,
                        revision,
                        output.parent / "screenshots" / engine.value,
                        timeout_ms,
                        arguments.cancel,
                        arguments.cancel_grace_ms,
                        arguments.browser_heap_sample,
                        browser_name,
                        arguments.lifecycle_cycles,
                        device_scale_milli,
                        arguments.browser_memory_sample,
                        accessibility_probe=arguments.accessibility_probe,
                        require_reduced_motion=arguments.require_reduced_motion,
                        require_forced_colors=arguments.require_forced_colors,
                        asset_probe=arguments.asset_probe,
                        text_geometry_probe=arguments.text_geometry_probe,
                    )
        else:
            url = arguments.url
            if url is None:
                raise BrowserRuntimeError("browser URL was not provided")
            _validate_bridge_url(url, arguments.bridge)
            client = WebDriverClient(driver_url, arguments.timeout_seconds)
            if arguments.scenario == "canvas":
                trace = run_canvas_scenario(
                    client,
                    engine,
                    url,
                    revision,
                    output.parent / "screenshots" / engine.value / "canvas",
                    timeout_ms,
                    canvas_ids,
                    consumer_revision,
                    canvas_attributes,
                    browser_heap=arguments.browser_heap_sample,
                    browser_memory=arguments.browser_memory_sample,
                    keyboard_trace=keyboard_trace,
                    browser_name=browser_name,
                    device_scale_milli=device_scale_milli,
                )
            elif arguments.scenario == "fragment":
                trace = run_fragment_scenario(
                    client,
                    engine,
                    url,
                    revision,
                    output.parent / "screenshots" / engine.value / "fragment",
                    timeout_ms,
                    browser_heap=arguments.browser_heap_sample,
                    browser_memory=arguments.browser_memory_sample,
                    browser_name=browser_name,
                    device_scale_milli=device_scale_milli,
                )
            else:
                trace = run_scenario(
                    client,
                    engine,
                    url,
                    arguments.bridge,
                    revision,
                    output.parent / "screenshots" / engine.value,
                    timeout_ms,
                    arguments.cancel,
                    arguments.cancel_grace_ms,
                    arguments.browser_heap_sample,
                    browser_name,
                    arguments.lifecycle_cycles,
                    device_scale_milli,
                    arguments.browser_memory_sample,
                    accessibility_probe=arguments.accessibility_probe,
                    require_reduced_motion=arguments.require_reduced_motion,
                    require_forced_colors=arguments.require_forced_colors,
                    asset_probe=arguments.asset_probe,
                    text_geometry_probe=arguments.text_geometry_probe,
                )
        _write_trace(output, trace.document())
        print(json.dumps(trace.document(), sort_keys=True))
        return 0
    except (BrowserRuntimeError, OSError, ValueError) as error:
        failure: Dict[str, Any] = {"schema": 1, "status": "failed", "engine": engine.value, "error": str(error), "unsupported_native_operations": list(UNSUPPORTED_NATIVE_OPERATIONS)}
        if trace is not None:
            failure.update(trace.document(status="failed"))
        try:
            _write_trace(output, failure)
        except (BrowserRuntimeError, OSError) as write_error:
            print(f"cannot write browser failure trace: {write_error}", file=os.sys.stderr)
        print(f"browser runtime failed: {error}", file=os.sys.stderr)
        return 1
