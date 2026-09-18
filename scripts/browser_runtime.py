"""Run the Metis browser workbench through a W3C WebDriver endpoint.

The runner deliberately uses only the Python standard library.  Browser and
driver installation stays with the host or CI job; this file owns the bounded
action trace, semantic assertions, screenshots and teardown evidence shared by
Chromium, Firefox and WebKit.
"""
from __future__ import annotations

import json
import pathlib
import re
import urllib.parse
from typing import Any, Dict, List, Mapping, Optional

from browser_protocol import (
    ELEMENT_KEY,
    MAX_TRACE_BYTES,
    MAX_WAIT_MILLISECONDS,
    MAX_URL_BYTES,
    ROOT,
    BrowserRuntimeError,
    _bounded_text,
    _safe_path,
    WebDriverClient,
)
from browser_accessibility import capture_accessibility
from browser_trace import (
    BrowserEngine,
    Trace,
    UNSUPPORTED_NATIVE_OPERATIONS,
    browser_heap_sample,
    browser_memory_sample,
    record_device_scale,
    screenshot,
)
from browser_runtime_probes import capture_runtime_features
from browser_text_stability import validate_text_geometry_stability


BRIDGE_MODES = ("disconnected", "authorized")
MAX_LIFECYCLE_CYCLES = 8


WAIT_FOR_SCRIPT = """
const done = arguments[arguments.length - 1];
const selector = arguments[0];
const expected = arguments[1];
const include = arguments[2];
const limit = arguments[3];
const read = () => {
  const element = document.querySelector(selector);
  const text = element ? (element.textContent || '').trim() : '';
  return element !== null && (include ? text.includes(expected) : text === expected);
};
if (read()) { done({ok: true}); return; }
let settled = false;
const observer = new MutationObserver(() => {
  if (settled || !read()) return;
  settled = true;
  window.clearTimeout(timer);
  observer.disconnect();
  done({ok: true});
});
observer.observe(document, {subtree: true, childList: true, attributes: true, characterData: true});
const timer = window.setTimeout(() => {
  if (settled) return;
  settled = true;
  observer.disconnect();
  done({ok: false});
}, limit);
"""


QUIET_SCRIPT = """
const done = arguments[arguments.length - 1];
const limit = arguments[0];
window.setTimeout(() => done({ok: true}), limit);
"""


SNAPSHOT_SCRIPT = """
const ids = arguments[0];
const value = (id) => {
  const element = document.getElementById(id);
  if (!element) return null;
  return {
    text: (element.textContent || '').trim(),
    value: typeof element.value === 'string' ? element.value : null,
    disabled: element.disabled === true,
    busy: element.getAttribute('aria-busy'),
    theme: element.getAttribute('data-metis-theme'),
  };
};
const root = document.getElementById('metis-app');
return {
  app_text: root ? (root.textContent || '').trim() : '',
  mounted_controls: root ? root.querySelectorAll('input,select,textarea,button').length : 0,
  lifecycle: {
    listener_count: root ? Number(root.getAttribute('data-metis-listener-count')) : 0,
    generation: root ? Number(root.getAttribute('data-metis-generation')) : 0,
  },
  document_theme: document.body.getAttribute('data-metis-theme'),
  elements: Object.fromEntries(ids.map((id) => [id, value(id)])),
};
"""


def _wait_for_text(client: WebDriverClient, element_id: str, expected: str, *, include: bool, timeout_ms: int) -> None:
    """Wait on a DOM mutation rather than polling or sleeping in the host."""
    result = client.execute_async(WAIT_FOR_SCRIPT, [f"#{element_id}", expected, include, timeout_ms])
    if not isinstance(result, dict) or result.get("ok") is not True:
        relation = "contains" if include else "equals"
        raise BrowserRuntimeError(f"#{element_id} did not {relation} {expected!r} within {timeout_ms} ms")


def _wait_for_selector(client: WebDriverClient, selector: str, *, timeout_ms: int) -> None:
    """Wait for one mounted DOM selector using the same mutation observer."""
    _wait_for_text(client, selector.lstrip("#"), "", include=True, timeout_ms=timeout_ms)


def _wait_quiet(client: WebDriverClient, milliseconds: int) -> None:
    """Allow browser timers and a delayed response to settle without host polling."""
    if not 1 <= milliseconds <= MAX_WAIT_MILLISECONDS:
        raise BrowserRuntimeError(f"quiet interval must be between 1 and {MAX_WAIT_MILLISECONDS} milliseconds")
    result = client.execute_async(QUIET_SCRIPT, [milliseconds])
    if not isinstance(result, dict) or result.get("ok") is not True:
        raise BrowserRuntimeError("browser quiet interval did not settle")


def _snapshot(client: WebDriverClient, trace: Trace, label: str) -> Dict[str, Any]:
    """Capture bounded semantic state for the manual and diff tooling."""
    ids = [
        "metis-status",
        "metis-form",
        "patient-id",
        "weight-kg",
        "concentration-mg-ml",
        "target-dose",
        "submit-calculation",
        "result-state",
        "result-metrics",
        "result-weight",
        "result-dose",
    ]
    value = client.execute(SNAPSHOT_SCRIPT, [ids])
    if not isinstance(value, dict):
        raise BrowserRuntimeError(f"semantic snapshot {label!r} is not an object")
    lifecycle = value.get("lifecycle")
    if (
        not isinstance(lifecycle, dict)
        or type(lifecycle.get("listener_count")) is not int
        or lifecycle["listener_count"] < 0
        or type(lifecycle.get("generation")) is not int
        or lifecycle["generation"] < 1
    ):
        raise BrowserRuntimeError(f"semantic snapshot {label!r} has invalid lifecycle state")
    result = {"label": label, **value}
    trace.snapshots.append(result)
    return result


def _dispatch_change(client: WebDriverClient, element_id: str) -> None:
    """Deliver the delegated Rust change action after WebDriver input."""
    client.execute(
        "arguments[0].dispatchEvent(new Event('change', {bubbles: true}));",
        [{ELEMENT_KEY: element_id}],
    )


def run_scenario(
    client: WebDriverClient,
    engine: BrowserEngine,
    url: str,
    bridge: str,
    revision: str,
    screenshot_directory: pathlib.Path,
    timeout_ms: int,
    cancel: bool,
    cancel_grace_ms: int,
    browser_heap: bool = False,
    browser_name: Optional[str] = None,
    lifecycle_cycles: int = 1,
    device_scale_milli: Optional[int] = None,
    browser_memory: bool = False,
    accessibility_probe: bool = False,
    require_reduced_motion: bool = False,
    require_forced_colors: bool = False,
    asset_probe: bool = False,
    media_probe: bool = False,
    media_playback_probe: bool = False,
    font_probe: bool = False,
    text_geometry_probe: bool = False,
) -> Trace:
    """Execute the same input, bridge and bounded teardown trace for every engine."""
    if bridge not in BRIDGE_MODES:
        raise BrowserRuntimeError(f"unsupported bridge mode {bridge!r}; choose {', '.join(BRIDGE_MODES)}")
    if type(lifecycle_cycles) is not int or not 1 <= lifecycle_cycles <= MAX_LIFECYCLE_CYCLES:
        raise BrowserRuntimeError(
            f"lifecycle-cycles must be between 1 and {MAX_LIFECYCLE_CYCLES}"
        )
    if require_reduced_motion and not accessibility_probe:
        raise BrowserRuntimeError("--require-reduced-motion requires --accessibility-probe")
    if require_forced_colors and not accessibility_probe:
        raise BrowserRuntimeError("--require-forced-colors requires --accessibility-probe")
    trace: Optional[Trace] = None
    stopped_snapshot: Optional[Dict[str, Any]] = None
    remounted_snapshot: Optional[Dict[str, Any]] = None
    try:
        client.create_session(engine.resolve_webdriver_name(browser_name), device_scale_milli)
        client.set_timeouts(timeout_ms)
        trace = Trace(engine, url, bridge, revision, client.capabilities)

        def capture_features(label: str) -> None:
            capture_runtime_features(
                client,
                trace,
                label,
                asset_probe=asset_probe,
                media_probe=media_probe,
                media_playback_probe=media_playback_probe,
                font_probe=font_probe,
                text_geometry_probe=text_geometry_probe,
            )

        client.navigate(url)
        _wait_for_selector(client, "#metis-form", timeout_ms=timeout_ms)
        record_device_scale(client, trace, device_scale_milli)
        initial = _snapshot(client, trace, "initial")
        if browser_heap:
            browser_heap_sample(client, trace, "initial")
        if browser_memory:
            browser_memory_sample(client, trace, "initial")
        screenshot(client, trace, screenshot_directory, "initial")
        if bridge == "authorized":
            _wait_for_text(client, "metis-status", "Authorized backend session ready", include=True, timeout_ms=timeout_ms)
            trace.actions.append({"action": "await-authorized-bridge", "result": "ready"})
        accessibility_baseline: Optional[Dict[str, Any]] = None
        if accessibility_probe:
            accessibility_baseline = capture_accessibility(
                client,
                trace,
                "initial",
                require_reduced_motion=require_reduced_motion,
                require_forced_colors=require_forced_colors,
            )
        capture_features("initial")
        for element_id, value, expected in (
            ("weight-kg", "80", "80.00 kg"),
            ("target-dose", "0.75", "0.750 mcg/kg/min"),
        ):
            element = client.find(f"#{element_id}")
            client.clear(element)
            client.send_keys(element, value)
            _dispatch_change(client, element)
            _wait_for_text(client, "result-weight" if element_id == "weight-kg" else "result-dose", expected, include=False, timeout_ms=timeout_ms)
            trace.actions.append({"action": "input-change", "field": element_id, "value": value, "observed": expected})
            _snapshot(client, trace, f"after-{element_id}")
            if accessibility_probe:
                capture_accessibility(
                    client,
                    trace,
                    f"after-{element_id}",
                    baseline=accessibility_baseline,
                )
            if browser_heap:
                browser_heap_sample(client, trace, f"after-{element_id}")
            if browser_memory:
                browser_memory_sample(client, trace, f"after-{element_id}")
            screenshot(client, trace, screenshot_directory, f"after-{element_id}")

        if bridge == "authorized":
            submit = client.find("#submit-calculation")
            client.click(submit)
            if cancel:
                _wait_for_text(client, "result-state", "Request in progress", include=True, timeout_ms=timeout_ms)
                trace.actions.append({"action": "submit", "state": "pending"})
                client.click(client.find("#metis-stop"))
                _wait_for_text(client, "metis-app", "Metis browser host stopped.", include=True, timeout_ms=timeout_ms)
                stopped_snapshot = _snapshot(client, trace, "stopped-after-cancel")
                screenshot(client, trace, screenshot_directory, "stopped-after-cancel")
                _assert_stopped(stopped_snapshot)
                client.click(client.find("#metis-start"))
                _wait_for_selector(client, "#metis-form", timeout_ms=timeout_ms)
                _wait_quiet(client, cancel_grace_ms)
                remounted_snapshot = _snapshot(client, trace, "remounted-after-cancel")
                if accessibility_probe:
                    capture_accessibility(client, trace, "remounted-after-cancel")
                capture_features("remounted-after-cancel")
                if browser_heap:
                    browser_heap_sample(client, trace, "remounted-after-cancel")
                if browser_memory:
                    browser_memory_sample(client, trace, "remounted-after-cancel")
                _assert_remount_has_no_result(remounted_snapshot)
                _assert_no_pending_request(remounted_snapshot)
                trace.actions.append({"action": "cancel-stop-remount", "stale_result": False})
            else:
                _wait_for_text(client, "result-state", "Backend result received", include=True, timeout_ms=timeout_ms)
                metrics = client.execute("return document.getElementById('result-metrics').textContent.trim();")
                if metrics != "Volume rate: 0.900000 mL/hr":
                    raise BrowserRuntimeError(f"authorized result differs: {metrics!r}")
                trace.actions.append({"action": "submit", "state": "success", "metrics": metrics})
                _snapshot(client, trace, "success")
                if browser_heap:
                    browser_heap_sample(client, trace, "success")
                if browser_memory:
                    browser_memory_sample(client, trace, "success")
                screenshot(client, trace, screenshot_directory, "success")
        else:
            submit_state = initial["elements"]["submit-calculation"]
            if not isinstance(submit_state, dict) or submit_state.get("disabled") is not True:
                raise BrowserRuntimeError("disconnected workbench exposed an enabled privileged submit control")
            trace.actions.append({"action": "disconnected-submit", "state": "disabled"})

        if not cancel or bridge != "authorized":
            client.click(client.find("#metis-stop"))
            _wait_for_text(client, "metis-app", "Metis browser host stopped.", include=True, timeout_ms=timeout_ms)
            stopped_snapshot = _snapshot(client, trace, "stopped")
            screenshot(client, trace, screenshot_directory, "stopped")
            _assert_stopped(stopped_snapshot)
            client.click(client.find("#metis-start"))
            _wait_for_selector(client, "#metis-form", timeout_ms=timeout_ms)
            remounted_snapshot = _snapshot(client, trace, "remounted")
            if accessibility_probe:
                capture_accessibility(client, trace, "remounted")
            capture_features("remounted")
            if browser_heap:
                browser_heap_sample(client, trace, "remounted")
            if browser_memory:
                browser_memory_sample(client, trace, "remounted")
            _assert_remount_has_no_result(remounted_snapshot)
            _assert_no_pending_request(remounted_snapshot)
            screenshot(client, trace, screenshot_directory, "remounted")
            trace.actions.append({"action": "stop-remount", "stale_result": False})

        if stopped_snapshot is None or remounted_snapshot is None:
            raise BrowserRuntimeError("browser lifecycle trace did not collect both teardown snapshots")
        _assert_lifecycle_transition(stopped_snapshot, remounted_snapshot)
        if text_geometry_probe:
            trace.metrics["text_geometry_stability"] = validate_text_geometry_stability(
                trace.metrics.get("text_geometry")
            )
        lifecycle_records = [_lifecycle_record(1, stopped_snapshot, remounted_snapshot)]
        for cycle in range(2, lifecycle_cycles + 1):
            client.click(client.find("#metis-stop"))
            _wait_for_text(client, "metis-app", "Metis browser host stopped.", include=True, timeout_ms=timeout_ms)
            cycle_stopped = _snapshot(client, trace, f"stopped-cycle-{cycle}")
            _assert_stopped(cycle_stopped)
            client.click(client.find("#metis-start"))
            _wait_for_selector(client, "#metis-form", timeout_ms=timeout_ms)
            cycle_remounted = _snapshot(client, trace, f"remounted-cycle-{cycle}")
            if accessibility_probe:
                capture_accessibility(client, trace, f"remounted-cycle-{cycle}")
            capture_features(f"remounted-cycle-{cycle}")
            if browser_heap:
                browser_heap_sample(client, trace, f"remounted-cycle-{cycle}")
            if browser_memory:
                browser_memory_sample(client, trace, f"remounted-cycle-{cycle}")
            _assert_remount_has_no_result(cycle_remounted)
            _assert_no_pending_request(cycle_remounted)
            _assert_lifecycle_transition(cycle_stopped, cycle_remounted)
            lifecycle_records.append(_lifecycle_record(cycle, cycle_stopped, cycle_remounted))
            trace.actions.append({"action": "stop-remount", "cycle": cycle, "stale_result": False})
        trace.metrics["lifecycle_cycles"] = lifecycle_records
        trace.cleanup = {
            "session_closed": False,
            "lifecycle_cycles": len(lifecycle_records),
            "stopped_message": True,
            "stopped_mounted_controls": stopped_snapshot["mounted_controls"],
            "remounted_mounted_controls": remounted_snapshot["mounted_controls"],
            "stopped_listener_count": stopped_snapshot["lifecycle"]["listener_count"],
            "remounted_listener_count": remounted_snapshot["lifecycle"]["listener_count"],
            "stopped_generation": stopped_snapshot["lifecycle"]["generation"],
            "remounted_generation": remounted_snapshot["lifecycle"]["generation"],
            "final_generation": lifecycle_records[-1]["remounted"]["generation"],
            "pending_requests": 0,
            "pending_request_observation": "remounted metis-form aria-busy=false",
            "listeners_or_tasks": "Metis-owned listener handles released at stop; provider-private resources remain outside WebDriver",
        }
        return trace
    finally:
        client.close()
        if trace is not None:
            trace.cleanup["session_closed"] = True


def _assert_stopped(snapshot: Mapping[str, Any]) -> None:
    """Require the stopped root to contain no mounted application controls."""
    lifecycle = snapshot.get("lifecycle")
    app_text = snapshot.get("app_text")
    if (
        snapshot.get("mounted_controls") != 0
        or not isinstance(app_text, str)
        or not app_text.endswith("Metis browser host stopped.")
        or not isinstance(lifecycle, dict)
        or lifecycle.get("listener_count") != 0
    ):
        raise BrowserRuntimeError(f"stopped DOM retained application state: {snapshot}")


def _assert_remount_has_no_result(snapshot: Mapping[str, Any]) -> None:
    """Reject a response delivered to a new lifecycle generation."""
    lifecycle = snapshot.get("lifecycle")
    if (
        not isinstance(lifecycle, dict)
        or not isinstance(lifecycle.get("listener_count"), int)
        or lifecycle["listener_count"] <= 0
    ):
        raise BrowserRuntimeError("remounted browser generation did not restore Rust-owned listeners")
    mounted_controls = snapshot.get("mounted_controls")
    if type(mounted_controls) is not int or mounted_controls <= 0:
        raise BrowserRuntimeError("remounted browser generation did not restore mounted controls")
    elements = snapshot.get("elements")
    result = elements.get("result-state") if isinstance(elements, dict) else None
    if not isinstance(result, dict) or result.get("text") == "Backend result received":
        raise BrowserRuntimeError("remounted browser generation retained a stale backend result")


def _assert_no_pending_request(snapshot: Mapping[str, Any]) -> None:
    """Require the remounted public form to report an idle request state."""
    elements = snapshot.get("elements")
    form = elements.get("metis-form") if isinstance(elements, dict) else None
    if not isinstance(form, dict) or form.get("busy") != "false":
        raise BrowserRuntimeError("remounted browser generation did not report aria-busy=false")


def _assert_lifecycle_transition(stopped: Mapping[str, Any], remounted: Mapping[str, Any]) -> None:
    """Require stop to release listeners and remount to advance its generation."""
    stopped_lifecycle = stopped.get("lifecycle")
    remounted_lifecycle = remounted.get("lifecycle")
    if not isinstance(stopped_lifecycle, dict) or not isinstance(remounted_lifecycle, dict):
        raise BrowserRuntimeError("browser lifecycle snapshots omitted lifecycle state")
    stopped_generation = stopped_lifecycle.get("generation")
    remounted_generation = remounted_lifecycle.get("generation")
    if not isinstance(stopped_generation, int) or not isinstance(remounted_generation, int):
        raise BrowserRuntimeError("browser lifecycle generations were not integers")
    if remounted_generation <= stopped_generation:
        raise BrowserRuntimeError("browser remount did not advance its lifecycle generation")


def _lifecycle_record(
    cycle: int,
    stopped: Mapping[str, Any],
    remounted: Mapping[str, Any],
) -> Dict[str, Any]:
    """Return the bounded semantic evidence for one stop/remount cycle."""
    stopped_lifecycle = stopped["lifecycle"]
    remounted_lifecycle = remounted["lifecycle"]
    return {
        "cycle": cycle,
        "stopped": {
            "generation": stopped_lifecycle["generation"],
            "listener_count": stopped_lifecycle["listener_count"],
            "mounted_controls": stopped["mounted_controls"],
        },
        "remounted": {
            "generation": remounted_lifecycle["generation"],
            "listener_count": remounted_lifecycle["listener_count"],
            "mounted_controls": remounted["mounted_controls"],
        },
    }


def _write_trace(path: pathlib.Path, document: Mapping[str, Any]) -> None:
    """Write one bounded, deterministic JSON trace."""
    _safe_path(path, directory=ROOT / "output")
    path.parent.mkdir(parents=True, exist_ok=True)
    encoded = json.dumps(document, indent=2, sort_keys=True).encode("utf-8")
    if len(encoded) > MAX_TRACE_BYTES:
        raise BrowserRuntimeError("browser trace exceeds the 512 KiB budget")
    path.write_bytes(encoded + b"\n")


def _validate_bridge_url(url: str, bridge: str) -> None:
    """Require the host-provided session tuple for an authorized run."""
    if bridge != "authorized":
        return
    _bounded_text(url, "browser URL", MAX_URL_BYTES)
    query = urllib.parse.parse_qs(urllib.parse.urlsplit(url).query, keep_blank_values=True)
    missing = [name for name in ("endpoint", "process", "principal") if not query.get(name, [""])[0]]
    if missing:
        raise BrowserRuntimeError(
            "authorized browser runs require URL query values: " + ", ".join(missing)
        )
    endpoint = query["endpoint"][0]
    endpoint_parts = urllib.parse.urlsplit(endpoint)
    if endpoint_parts.scheme not in {"ws", "wss"} or not endpoint_parts.netloc:
        raise BrowserRuntimeError("authorized browser endpoint must be a ws(s) URL")
    if not re.fullmatch(r"[0-9]+", query["process"][0]):
        raise BrowserRuntimeError("authorized browser process must be decimal digits")
    if not re.fullmatch(r"[0-9a-fA-F]{32}", query["principal"][0]):
        raise BrowserRuntimeError("authorized browser principal must be 32 hexadecimal digits")

if __name__ == "__main__":
    from browser_runtime_cli import main
    raise SystemExit(main())
