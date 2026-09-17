"""Measure browser input delivery to the next animation-frame boundary.

The instrument records the interval from one trusted ``click`` event to the
following ``requestAnimationFrame`` callback. It describes the browser event
and frame boundary only; operating-system compositor, GPU and native-window
latency remain separate measurements.
"""
from __future__ import annotations

import argparse
import json
import math
import pathlib
import statistics
import subprocess
from typing import Any, Mapping

from browser_protocol import (
    MAX_TRACE_BYTES,
    ROOT,
    BrowserRuntimeError,
    WebDriverClient,
    _bounded_text,
    _safe_path,
    parse_device_scale,
)
from browser_trace import BrowserEngine, Trace, record_device_scale


MAX_SAMPLES = 32
DEFAULT_SAMPLES = 3
MAX_TIMEOUT_MILLISECONDS = 120_000
MAX_TOTAL_TIMEOUT_MILLISECONDS = 300_000
MAX_LATENCY_MILLISECONDS = 120_000.0
MAX_SELECTOR_BYTES = 256
INPUT_EVENT = "click"


INSTALL_SCRIPT = r"""
const selector = arguments[0];
const eventType = arguments[1];
const maxSamples = arguments[2];
if (window.__metisInputLatencyState) {
  return {ok: false, error: "input latency trace is already installed"};
}
if (!window.performance || typeof window.performance.now !== "function" ||
    typeof window.requestAnimationFrame !== "function") {
  return {ok: false, error: "browser timing APIs are unavailable"};
}
let target;
try { target = document.querySelector(selector); }
catch (_) { return {ok: false, error: "input latency selector is invalid"}; }
if (!target) return {ok: false, error: "input latency target is absent"};
const events = [];
const listener = (event) => {
  if (events.length >= maxSamples) return;
  const targetElement = event.target;
  if (!(targetElement instanceof Element)) return;
  let matched;
  try { matched = targetElement.closest(selector); }
  catch (_) { return; }
  if (matched !== target) return;
  const eventTimestamp = window.performance.now();
  window.requestAnimationFrame((frameTimestamp) => {
    if (events.length >= maxSamples) return;
    const frame = Number.isFinite(frameTimestamp)
      ? frameTimestamp : window.performance.now();
    events.push({
      type: event.type,
      is_trusted: event.isTrusted === true,
      target_id: typeof targetElement.id === "string" && targetElement.id
        ? targetElement.id : null,
      matches_selector: true,
      event_ms: eventTimestamp,
      frame_ms: frame,
      latency_ms: frame - eventTimestamp,
    });
  });
};
target.addEventListener(eventType, listener, {capture: true, passive: true});
window.__metisInputLatencyState = {events, target, eventType, listener};
return {ok: true, listener_count: 1};
"""


WAIT_SCRIPT = r"""
const done = arguments[arguments.length - 1];
const expected = arguments[0];
const timeout = arguments[1];
const state = window.__metisInputLatencyState;
if (!state) { done({ok: false, error: "input latency trace is absent", events: []}); return; }
const deadline = window.performance.now() + timeout;
let timer;
const finish = (value) => {
  window.clearTimeout(timer);
  done(value);
};
const poll = () => {
  if (state.events.length >= expected) {
    finish({ok: true, events: state.events.splice(0, expected)});
    return;
  }
  if (window.performance.now() >= deadline) {
    finish({ok: false, error: "input latency deadline exceeded", events: state.events.splice(0)});
    return;
  }
  timer = window.setTimeout(poll, 0);
};
poll();
"""


CLEANUP_SCRIPT = r"""
const state = window.__metisInputLatencyState;
if (!state) return {ok: true, listener_count: 0};
state.target.removeEventListener(state.eventType, state.listener, true);
delete window.__metisInputLatencyState;
return {ok: true, listener_count: 1};
"""


def validate_selector(value: Any) -> str:
    """Validate one bounded CSS selector supplied by the caller."""
    if not isinstance(value, str) or not value.strip():
        raise BrowserRuntimeError("input latency selector must be non-empty text")
    selector = _bounded_text(value.strip(), "input latency selector", MAX_SELECTOR_BYTES)
    if any(ord(character) < 0x20 for character in selector):
        raise BrowserRuntimeError("input latency selector contains a control character")
    return selector


def validate_samples(value: Any) -> int:
    """Validate the number of repeated input observations."""
    if type(value) is not int or not 2 <= value <= MAX_SAMPLES:
        raise BrowserRuntimeError(f"input latency samples must be between 2 and {MAX_SAMPLES}")
    return value


def validate_timeout(value: Any) -> int:
    """Validate one bounded wait for the event-to-frame observation."""
    if type(value) is not int or not 1 <= value <= MAX_TIMEOUT_MILLISECONDS:
        raise BrowserRuntimeError(
            f"input latency timeout must be between 1 and {MAX_TIMEOUT_MILLISECONDS} milliseconds"
        )
    return value


def _validate_sample(value: Any) -> dict[str, Any]:
    """Validate one browser event/frame observation."""
    if not isinstance(value, Mapping):
        raise BrowserRuntimeError("input latency sample is not an object")
    if value.get("type") != INPUT_EVENT or value.get("is_trusted") is not True:
        raise BrowserRuntimeError("input latency sample was not a trusted click")
    if value.get("matches_selector") is not True:
        raise BrowserRuntimeError("input latency sample targeted the wrong element")
    target_id = value.get("target_id")
    if target_id is not None and (
        not isinstance(target_id, str) or not target_id or len(target_id.encode("utf-8")) > MAX_SELECTOR_BYTES
    ):
        raise BrowserRuntimeError("input latency sample has an invalid target id")
    numbers: dict[str, float] = {}
    for name in ("event_ms", "frame_ms", "latency_ms"):
        raw = value.get(name)
        if isinstance(raw, bool) or not isinstance(raw, (int, float)):
            raise BrowserRuntimeError(f"input latency sample has invalid {name}")
        number = float(raw)
        if not math.isfinite(number) or number < 0.0:
            raise BrowserRuntimeError(f"input latency sample has invalid {name}")
        numbers[name] = number
    if numbers["latency_ms"] > MAX_LATENCY_MILLISECONDS:
        raise BrowserRuntimeError("input latency sample exceeds its bound")
    derived = numbers["frame_ms"] - numbers["event_ms"]
    if derived < 0.0 or not math.isclose(derived, numbers["latency_ms"], rel_tol=0.0, abs_tol=1e-6):
        raise BrowserRuntimeError("input latency sample has inconsistent timestamps")
    return {
        "type": INPUT_EVENT,
        "is_trusted": True,
        "target_id": target_id,
        "event_ms": numbers["event_ms"],
        "frame_ms": numbers["frame_ms"],
        "latency_ms": numbers["latency_ms"],
    }


def _wait_for_sample(client: WebDriverClient, timeout_ms: int) -> dict[str, Any]:
    """Wait for one event-to-frame observation and validate its shape."""
    value = client.execute_async(WAIT_SCRIPT, [1, timeout_ms])
    if not isinstance(value, Mapping) or value.get("ok") is not True:
        detail = value.get("error") if isinstance(value, Mapping) else value
        raise BrowserRuntimeError(f"input latency observation failed: {detail!r}")
    events = value.get("events")
    if not isinstance(events, list) or len(events) != 1:
        raise BrowserRuntimeError("input latency observation returned an invalid event count")
    return _validate_sample(events[0])


def measure_input_latency(
    client: WebDriverClient,
    trace: Trace,
    selector: str,
    *,
    samples: int = DEFAULT_SAMPLES,
    timeout_ms: int = 4_000,
) -> Mapping[str, Any]:
    """Measure trusted clicks from one stable element to the next frame."""
    selector = validate_selector(selector)
    samples = validate_samples(samples)
    timeout_ms = validate_timeout(timeout_ms)
    if samples * timeout_ms > MAX_TOTAL_TIMEOUT_MILLISECONDS:
        raise BrowserRuntimeError("input latency sample budget exceeds five minutes")
    installed = client.execute(INSTALL_SCRIPT, [selector, INPUT_EVENT, samples])
    if not isinstance(installed, Mapping) or installed.get("ok") is not True:
        detail = installed.get("error") if isinstance(installed, Mapping) else installed
        raise BrowserRuntimeError(f"input latency trace could not be installed: {detail!r}")
    if installed.get("listener_count") != 1:
        raise BrowserRuntimeError("input latency trace installed an unexpected listener count")
    try:
        observations = []
        for index in range(samples):
            element = client.find(selector)
            client.click(element)
            sample = _wait_for_sample(client, timeout_ms)
            trace.actions.append(
                {
                    "action": "trusted-click",
                    "selector": selector,
                    "sample": index + 1,
                    "observed_event": sample,
                }
            )
            observations.append(sample)
        latencies = [sample["latency_ms"] for sample in observations]
        measurement = {
            "selector": selector,
            "event": INPUT_EVENT,
            "semantic": "trusted event to next requestAnimationFrame callback",
            "sample_count": len(latencies),
            "latencies_ms": latencies,
            "mean_ms": statistics.fmean(latencies),
            "stddev_ms": statistics.pstdev(latencies),
            "min_ms": min(latencies),
            "max_ms": max(latencies),
        }
        trace.metrics["input_to_frame_latency"] = measurement
        return measurement
    finally:
        cleanup = client.execute(CLEANUP_SCRIPT)
        if not isinstance(cleanup, Mapping) or cleanup.get("ok") is not True or cleanup.get("listener_count") != 1:
            raise BrowserRuntimeError("input latency trace cleanup released an unexpected listener count")


def _write_trace(path: pathlib.Path, document: Mapping[str, Any]) -> None:
    """Write one bounded JSON trace below the repository output root."""
    path = _safe_path(path, directory=ROOT / "output")
    encoded = json.dumps(document, indent=2, sort_keys=True).encode("utf-8")
    if len(encoded) > MAX_TRACE_BYTES:
        raise BrowserRuntimeError("input latency trace exceeds the 512 KiB budget")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(encoded + b"\n")


def build_parser() -> argparse.ArgumentParser:
    """Build the bounded input-latency command line."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--driver-url", required=True)
    parser.add_argument("--url", required=True)
    parser.add_argument("--engine", choices=tuple(engine.value for engine in BrowserEngine), default="chromium")
    parser.add_argument("--browser-name", choices=("chrome", "MicrosoftEdge", "firefox", "safari"))
    parser.add_argument("--device-scale", help="requested browser device scale between 0.5 and 4")
    parser.add_argument("--selector", required=True, help="stable CSS selector receiving trusted clicks")
    parser.add_argument("--samples", type=int, default=DEFAULT_SAMPLES)
    parser.add_argument("--timeout-ms", type=int, default=4_000)
    parser.add_argument("--width", type=int, default=1_440)
    parser.add_argument("--height", type=int, default=1_100)
    parser.add_argument("--headless", action="store_true")
    parser.add_argument("--output", type=pathlib.Path, default=ROOT / "output" / "browser" / "input-latency.json")
    return parser


def run(args: argparse.Namespace) -> int:
    """Run the real WebDriver instrument and persist its result."""
    engine = BrowserEngine.parse(args.engine)
    browser_name = engine.resolve_webdriver_name(args.browser_name)
    selector = validate_selector(args.selector)
    samples = validate_samples(args.samples)
    timeout_ms = validate_timeout(args.timeout_ms)
    if samples * timeout_ms > MAX_TOTAL_TIMEOUT_MILLISECONDS:
        raise BrowserRuntimeError("input latency sample budget exceeds five minutes")
    device_scale_milli = parse_device_scale(args.device_scale) if args.device_scale is not None else None
    revision = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True, timeout=30
    ).strip()
    trace: Trace | None = None
    result: Mapping[str, Any] | None = None
    exit_code = 1
    failure: BaseException | None = None
    client = WebDriverClient(args.driver_url, max(30.0, timeout_ms / 1000.0 + 5.0))
    try:
        client.create_session(browser_name, device_scale_milli, headless=args.headless)
        client.set_timeouts(timeout_ms)
        client.set_window_rect(args.width, args.height)
        trace = Trace(engine, args.url, "input-latency", revision, client.capabilities)
        client.navigate(args.url)
        record_device_scale(client, trace, device_scale_milli)
        measure_input_latency(client, trace, selector, samples=samples, timeout_ms=timeout_ms)
        trace.cleanup = {
            "session_closed": False,
            "input_listener": "released after final sample",
            "provider_latency": "unavailable from WebDriver",
        }
        result = trace.document()
        exit_code = 0
    except (BrowserRuntimeError, OSError, ValueError, subprocess.SubprocessError) as error:
        failure = error
    finally:
        try:
            client.close()
        except BrowserRuntimeError as close_error:
            if failure is None:
                failure = close_error
                exit_code = 1
            else:
                failure.add_note(f"browser session close also failed: {close_error}")
        if trace is not None:
            trace.cleanup["session_closed"] = client.session_id is None
            if failure is not None:
                result = trace.document(status="failed")
                result["error"] = str(failure)
        elif failure is not None:
            result = {
                "schema": 1,
                "status": "failed",
                "engine": engine.value,
                "error": str(failure),
            }
    if result is None:
        raise BrowserRuntimeError("input latency run produced no result")
    try:
        _write_trace(args.output, result)
    except (BrowserRuntimeError, OSError) as write_error:
        if failure is None:
            raise
        failure.add_note(f"input latency trace write also failed: {write_error}")
    if exit_code == 0:
        print(json.dumps(result, sort_keys=True))
    else:
        print(f"browser input latency failed: {failure}")
    return exit_code


def main(argv: list[str] | None = None) -> int:
    """Parse arguments and execute the instrument."""
    return run(build_parser().parse_args(argv))


if __name__ == "__main__":
    raise SystemExit(main())
