"""Measure browser navigation startup through readiness and first frame.

The instrument uses Navigation Timing for the page navigation milestones, a
caller-selected readiness element, and the following ``requestAnimationFrame``
callback. It measures the browser page boundary only; browser-process launch,
operating-system, compositor, GPU and native-window costs remain separate.
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


MAX_SAMPLES = 8
DEFAULT_SAMPLES = 3
MAX_TIMEOUT_MILLISECONDS = 120_000
MAX_TOTAL_TIMEOUT_MILLISECONDS = 300_000
MAX_MILESTONE_MILLISECONDS = 120_000.0
MAX_SELECTOR_BYTES = 256


NAVIGATION_SCRIPT = r"""
const entries = typeof performance.getEntriesByType === "function"
  ? performance.getEntriesByType("navigation") : [];
if (!Array.isArray(entries) || entries.length !== 1) {
  return {ok: false, error: "navigation timing is unavailable"};
}
const entry = entries[0];
const fields = [
  "startTime", "responseEnd", "domInteractive", "domContentLoadedEventEnd",
  "loadEventEnd", "duration",
];
for (const field of fields) {
  if (typeof entry[field] !== "number" || !Number.isFinite(entry[field])) {
    return {ok: false, error: `navigation timing field ${field} is invalid`};
  }
}
return {
  ok: true,
  entry_type: entry.entryType,
  start_ms: entry.startTime,
  response_end_ms: entry.responseEnd,
  dom_interactive_ms: entry.domInteractive,
  dom_content_loaded_ms: entry.domContentLoadedEventEnd,
  load_event_end_ms: entry.loadEventEnd,
  duration_ms: entry.duration,
  ready_state: document.readyState,
};
"""


READY_SCRIPT = r"""
const done = arguments[arguments.length - 1];
const selector = arguments[0];
const timeout = arguments[1];
if (!window.performance || typeof window.performance.now !== "function") {
  done({ok: false, error: "browser performance clock is unavailable"});
  return;
}
let settled = false;
let observer;
let timer;
const finish = (value) => {
  if (settled) return;
  settled = true;
  if (observer) observer.disconnect();
  window.clearTimeout(timer);
  done(value);
};
const read = () => {
  try {
    return {element: document.querySelector(selector), error: null};
  } catch (_) {
    return {element: null, error: "readiness selector is invalid"};
  }
};
const current = read();
if (current.error) {
  finish({ok: false, error: current.error});
  return;
}
if (current.element) {
  finish({
    ok: true,
    ready_ms: window.performance.now(),
    ready_state: document.readyState,
  });
  return;
}
observer = new MutationObserver(() => {
  const next = read();
  if (next.error) finish({ok: false, error: next.error});
  else if (next.element) {
    finish({
      ok: true,
      ready_ms: window.performance.now(),
      ready_state: document.readyState,
    });
  }
});
observer.observe(document, {subtree: true, childList: true, attributes: true});
timer = window.setTimeout(
  () => finish({ok: false, error: "readiness selector deadline exceeded"}),
  timeout
);
"""


FRAME_SCRIPT = r"""
const done = arguments[arguments.length - 1];
const timeout = arguments[0];
if (!window.performance || typeof window.performance.now !== "function" ||
    typeof window.requestAnimationFrame !== "function") {
  done({ok: false, error: "browser animation-frame timing is unavailable"});
  return;
}
let settled = false;
let timer;
const finish = (value) => {
  if (settled) return;
  settled = true;
  window.clearTimeout(timer);
  done(value);
};
timer = window.setTimeout(
  () => finish({ok: false, error: "first-frame deadline exceeded"}),
  timeout
);
window.requestAnimationFrame((timestamp) => {
  const frame = Number.isFinite(timestamp) ? timestamp : window.performance.now();
  finish({ok: true, frame_ms: frame});
});
"""


def validate_selector(value: Any) -> str:
    """Validate one bounded CSS selector supplied by the caller."""
    if not isinstance(value, str) or not value.strip():
        raise BrowserRuntimeError("startup readiness selector must be non-empty text")
    selector = _bounded_text(value.strip(), "startup readiness selector", MAX_SELECTOR_BYTES)
    if any(ord(character) < 0x20 for character in selector):
        raise BrowserRuntimeError("startup readiness selector contains a control character")
    return selector


def validate_samples(value: Any) -> int:
    """Validate the number of repeated page navigations."""
    if type(value) is not int or not 2 <= value <= MAX_SAMPLES:
        raise BrowserRuntimeError(f"startup latency samples must be between 2 and {MAX_SAMPLES}")
    return value


def validate_timeout(value: Any) -> int:
    """Validate one bounded readiness or first-frame wait."""
    if type(value) is not int or not 1 <= value <= MAX_TIMEOUT_MILLISECONDS:
        raise BrowserRuntimeError(
            "startup latency timeout must be between 1 and "
            f"{MAX_TIMEOUT_MILLISECONDS} milliseconds"
        )
    return value


def validate_budget(samples: int, timeout_ms: int) -> None:
    """Reject a pair of waits whose total browser budget exceeds five minutes."""
    if samples * timeout_ms * 2 > MAX_TOTAL_TIMEOUT_MILLISECONDS:
        raise BrowserRuntimeError("startup latency sample budget exceeds five minutes")


def _finite_milliseconds(value: Any, name: str) -> float:
    """Validate one non-negative bounded browser timestamp."""
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise BrowserRuntimeError(f"startup latency sample has invalid {name}")
    number = float(value)
    if not math.isfinite(number) or number < 0.0 or number > MAX_MILESTONE_MILLISECONDS:
        raise BrowserRuntimeError(f"startup latency sample has invalid {name}")
    return number


def _validate_navigation(value: Any) -> dict[str, Any]:
    """Validate Navigation Timing milestones and their ordering."""
    if not isinstance(value, Mapping) or value.get("ok") is not True:
        detail = value.get("error") if isinstance(value, Mapping) else value
        raise BrowserRuntimeError(f"navigation timing observation failed: {detail!r}")
    if value.get("entry_type") != "navigation":
        raise BrowserRuntimeError("navigation timing returned an unexpected entry type")
    names = (
        "start_ms",
        "response_end_ms",
        "dom_interactive_ms",
        "dom_content_loaded_ms",
        "load_event_end_ms",
        "duration_ms",
    )
    numbers = {name: _finite_milliseconds(value.get(name), name) for name in names}
    ordered = [
        numbers["start_ms"],
        numbers["response_end_ms"],
        numbers["dom_interactive_ms"],
        numbers["dom_content_loaded_ms"],
        numbers["load_event_end_ms"],
    ]
    if any(later < earlier for earlier, later in zip(ordered, ordered[1:])):
        raise BrowserRuntimeError("navigation timing milestones are not monotonic")
    if numbers["duration_ms"] < ordered[-1]:
        raise BrowserRuntimeError("navigation timing duration ends before load")
    ready_state = value.get("ready_state")
    if ready_state not in {"interactive", "complete"}:
        raise BrowserRuntimeError("navigation timing returned an invalid document state")
    return {
        "entry_type": "navigation",
        **numbers,
        "ready_state": ready_state,
    }


def _validate_ready(value: Any) -> dict[str, Any]:
    """Validate the readiness selector observation."""
    if not isinstance(value, Mapping) or value.get("ok") is not True:
        detail = value.get("error") if isinstance(value, Mapping) else value
        raise BrowserRuntimeError(f"startup readiness observation failed: {detail!r}")
    ready_ms = _finite_milliseconds(value.get("ready_ms"), "ready_ms")
    ready_state = value.get("ready_state")
    if ready_state not in {"interactive", "complete"}:
        raise BrowserRuntimeError("startup readiness returned an invalid document state")
    return {"ready_ms": ready_ms, "ready_state": ready_state}


def _validate_frame(value: Any) -> float:
    """Validate the first animation-frame timestamp."""
    if not isinstance(value, Mapping) or value.get("ok") is not True:
        detail = value.get("error") if isinstance(value, Mapping) else value
        raise BrowserRuntimeError(f"startup first-frame observation failed: {detail!r}")
    return _finite_milliseconds(value.get("frame_ms"), "frame_ms")


def _wait_ready(client: WebDriverClient, selector: str, timeout_ms: int) -> dict[str, Any]:
    """Wait for one caller-selected readiness element without host polling."""
    return _validate_ready(client.execute_async(READY_SCRIPT, [selector, timeout_ms]))


def _wait_frame(client: WebDriverClient, timeout_ms: int) -> float:
    """Wait for the next browser animation frame."""
    return _validate_frame(client.execute_async(FRAME_SCRIPT, [timeout_ms]))


def _summary(values: list[float]) -> dict[str, float]:
    """Return bounded descriptive statistics for one milestone series."""
    return {
        "mean_ms": statistics.fmean(values),
        "stddev_ms": statistics.pstdev(values),
        "min_ms": min(values),
        "max_ms": max(values),
    }


def measure_startup_latency(
    client: WebDriverClient,
    trace: Trace,
    url: str,
    selector: str,
    *,
    samples: int = DEFAULT_SAMPLES,
    timeout_ms: int = 4_000,
) -> Mapping[str, Any]:
    """Measure repeated navigation-to-ready and navigation-to-frame intervals."""
    selector = validate_selector(selector)
    samples = validate_samples(samples)
    timeout_ms = validate_timeout(timeout_ms)
    validate_budget(samples, timeout_ms)
    observations: list[dict[str, Any]] = []
    for index in range(samples):
        client.navigate(url)
        navigation = _validate_navigation(client.execute(NAVIGATION_SCRIPT))
        ready = _wait_ready(client, selector, timeout_ms)
        frame_ms = _wait_frame(client, timeout_ms)
        if ready["ready_ms"] < navigation["start_ms"]:
            raise BrowserRuntimeError("startup readiness precedes navigation start")
        if frame_ms < ready["ready_ms"]:
            raise BrowserRuntimeError("startup first frame precedes readiness observation")
        observation = {
            "navigation": navigation,
            "ready_ms": ready["ready_ms"],
            "ready_state": ready["ready_state"],
            "first_frame_ms": frame_ms,
        }
        trace.actions.append(
            {
                "action": "navigate",
                "sample": index + 1,
                "selector": selector,
                "observation": observation,
            }
        )
        observations.append(observation)
    ready_values = [item["ready_ms"] for item in observations]
    frame_values = [item["first_frame_ms"] for item in observations]
    metric: dict[str, Any] = {
        "selector": selector,
        "semantic": "navigation timing to readiness-selector observation and next requestAnimationFrame",
        "sample_count": len(observations),
        "samples": observations,
        "navigation_to_ready": _summary(ready_values),
        "navigation_to_first_frame": _summary(frame_values),
    }
    trace.metrics["startup_latency"] = metric
    return metric


def _write_trace(path: pathlib.Path, document: Mapping[str, Any]) -> None:
    """Write one bounded JSON trace below the repository output root."""
    path = _safe_path(path, directory=ROOT / "output")
    encoded = json.dumps(document, indent=2, sort_keys=True).encode("utf-8")
    if len(encoded) > MAX_TRACE_BYTES:
        raise BrowserRuntimeError("startup latency trace exceeds the 512 KiB budget")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(encoded + b"\n")


def build_parser() -> argparse.ArgumentParser:
    """Build the bounded startup-latency command line."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--driver-url", required=True)
    parser.add_argument("--url", required=True)
    parser.add_argument("--engine", choices=tuple(engine.value for engine in BrowserEngine), default="chromium")
    parser.add_argument("--browser-name", choices=("chrome", "MicrosoftEdge", "firefox", "safari"))
    parser.add_argument("--device-scale", help="requested browser device scale between 0.5 and 4")
    parser.add_argument("--ready-selector", required=True, help="stable CSS selector identifying app readiness")
    parser.add_argument("--samples", type=int, default=DEFAULT_SAMPLES)
    parser.add_argument("--timeout-ms", type=int, default=4_000)
    parser.add_argument("--width", type=int, default=1_440)
    parser.add_argument("--height", type=int, default=1_100)
    parser.add_argument("--headless", action="store_true")
    parser.add_argument(
        "--output",
        type=pathlib.Path,
        default=ROOT / "output" / "browser" / "startup-latency.json",
    )
    return parser


def run(args: argparse.Namespace) -> int:
    """Run the real WebDriver instrument and persist its result."""
    engine = BrowserEngine.parse(args.engine)
    browser_name = engine.resolve_webdriver_name(args.browser_name)
    selector = validate_selector(args.ready_selector)
    samples = validate_samples(args.samples)
    timeout_ms = validate_timeout(args.timeout_ms)
    validate_budget(samples, timeout_ms)
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
        trace = Trace(engine, args.url, "startup-latency", revision, client.capabilities)
        record_device_scale(client, trace, device_scale_milli)
        measure_startup_latency(
            client,
            trace,
            args.url,
            selector,
            samples=samples,
            timeout_ms=timeout_ms,
        )
        trace.cleanup = {
            "session_closed": False,
            "readiness_observers": "scoped observer removed after each sample",
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
        raise BrowserRuntimeError("startup latency run produced no result")
    try:
        _write_trace(args.output, result)
    except (BrowserRuntimeError, OSError) as write_error:
        if failure is None:
            raise
        failure.add_note(f"startup latency trace write also failed: {write_error}")
    if exit_code == 0:
        print(json.dumps(result, sort_keys=True))
    else:
        print(f"browser startup latency failed: {failure}")
    return exit_code


def main(argv: list[str] | None = None) -> int:
    """Parse arguments and execute the instrument."""
    return run(build_parser().parse_args(argv))


if __name__ == "__main__":
    raise SystemExit(main())
