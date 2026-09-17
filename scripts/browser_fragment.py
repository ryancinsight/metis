"""Run the authenticated, typed HTTP fragment browser trace."""
from __future__ import annotations

import pathlib
import re
from typing import Any, Dict, Mapping, Optional

from browser_protocol import BrowserRuntimeError, WebDriverClient
from browser_runtime import _wait_for_text
from browser_trace import (
    BrowserEngine,
    Trace,
    browser_heap_sample,
    browser_memory_sample,
    record_device_scale,
    screenshot,
)


FRAGMENT_BRIDGE = "http-fragment"
FRAGMENT_INPUT = "session"
FRAGMENT_SUCCESS = "Authenticated fragment boundary ready"
FRAGMENT_NEGATIVE = "malformed 400 · unauthorized 401 · target rejected · stale unchanged"
FRAGMENT_IDS = (
    "metis-status",
    "fragment-input",
    "metis-fragment",
    "metis-reset",
    "http-response",
    "metis-events",
    "metis-negative",
    "metis-lifecycle",
)
FRAGMENT_SNAPSHOT_SCRIPT = """
const ids = arguments[0];
const read = (id) => {
  const element = document.getElementById(id);
  if (!element) return null;
  return {
    text: (element.textContent || '').trim(),
    value: typeof element.value === 'string' ? element.value : null,
    disabled: element.disabled === true,
    busy: element.getAttribute('aria-busy'),
  };
};
return {
  app_text: (document.getElementById('metis-app')?.textContent || '').trim(),
  elements: Object.fromEntries(ids.map((id) => [id, read(id)])),
};
"""
FRAGMENT_GENERATION = re.compile(r"^generation ([1-9][0-9]*)$")


def _snapshot(client: WebDriverClient, trace: Trace, label: str) -> Dict[str, Any]:
    """Capture the bounded fragment page state for one trace point."""
    value = client.execute(FRAGMENT_SNAPSHOT_SCRIPT, [list(FRAGMENT_IDS)])
    if not isinstance(value, dict) or not isinstance(value.get("elements"), dict):
        raise BrowserRuntimeError(f"fragment snapshot {label!r} is malformed")
    elements = value["elements"]
    for element_id in FRAGMENT_IDS:
        element = elements.get(element_id)
        if not isinstance(element, dict) or not isinstance(element.get("text"), str):
            raise BrowserRuntimeError(f"fragment snapshot {label!r} omitted #{element_id}")
    lifecycle = elements["metis-lifecycle"]["text"]
    match = FRAGMENT_GENERATION.fullmatch(lifecycle)
    if match is None:
        raise BrowserRuntimeError(f"fragment snapshot {label!r} has invalid lifecycle {lifecycle!r}")
    busy = elements["metis-status"].get("busy")
    if busy != "false" or elements["metis-fragment"].get("disabled") is not False:
        raise BrowserRuntimeError(f"fragment snapshot {label!r} did not observe idle controls")
    result = {
        "label": label, **value, "generation": int(match.group(1)),
        "request_state": {"false": "idle"}[busy],
    }
    trace.snapshots.append(result)
    return result


def _text(snapshot: Mapping[str, Any], element_id: str) -> str:
    """Read one validated element text from a fragment snapshot."""
    elements = snapshot.get("elements")
    element = elements.get(element_id) if isinstance(elements, dict) else None
    value = element.get("text") if isinstance(element, dict) else None
    if not isinstance(value, str):
        raise BrowserRuntimeError(f"fragment snapshot omitted #{element_id}")
    return value


def _assert_ready(
    snapshot: Mapping[str, Any],
    *,
    expected_generation: int,
    require_negative: bool,
) -> None:
    """Require the authenticated result and the expected probe state."""
    if snapshot.get("generation") != expected_generation:
        raise BrowserRuntimeError("fragment lifecycle generation changed unexpectedly")
    if FRAGMENT_SUCCESS not in _text(snapshot, "metis-status"):
        raise BrowserRuntimeError("fragment page did not report an authenticated boundary")
    if _text(snapshot, "http-response") != (
        "200 metis-http-ready · handshake 200 · fragment 200 (1 patch)"
    ):
        raise BrowserRuntimeError("fragment page omitted the successful handshake or patch")
    event_text = _text(snapshot, "metis-events")
    if f"accepted: {FRAGMENT_INPUT}" not in event_text:
        raise BrowserRuntimeError("fragment page omitted the input-sensitive patch")
    negative = _text(snapshot, "metis-negative")
    if require_negative and negative != FRAGMENT_NEGATIVE:
        raise BrowserRuntimeError("fragment page omitted a required negative probe")
    if not require_negative and negative != "—":
        raise BrowserRuntimeError("fragment remount retained stale negative-probe state")


def run_fragment_scenario(
    client: WebDriverClient,
    engine: BrowserEngine,
    url: str,
    revision: str,
    screenshot_directory: pathlib.Path,
    timeout_ms: int,
    browser_heap: bool = False,
    browser_name: Optional[str] = None,
    device_scale_milli: Optional[int] = None,
    browser_memory: bool = False,
) -> Trace:
    """Exercise authenticated success, rejection, stale state and remount."""
    trace: Optional[Trace] = None
    try:
        client.create_session(engine.resolve_webdriver_name(browser_name), device_scale_milli)
        client.set_timeouts(timeout_ms)
        trace = Trace(engine, url, FRAGMENT_BRIDGE, revision, client.capabilities)
        client.navigate(url)
        record_device_scale(client, trace, device_scale_milli)
        _wait_for_text(client, "metis-status", FRAGMENT_SUCCESS, include=True, timeout_ms=timeout_ms)
        _wait_for_text(client, "metis-negative", FRAGMENT_NEGATIVE, include=False, timeout_ms=timeout_ms)
        ready = _snapshot(client, trace, "authenticated-success")
        _assert_ready(
            ready,
            expected_generation=ready["generation"],
            require_negative=True,
        )
        trace.actions.extend(
            [
                {"action": "health", "status": 200},
                {"action": "authenticated-fragment", "status": 200, "patches": 1},
                {"action": "negative-probes", "malformed": 400, "unauthorized": 401, "target": "rejected", "stale": "unchanged"},
            ]
        )
        if browser_heap:
            browser_heap_sample(client, trace, "authenticated-success")
        if browser_memory:
            browser_memory_sample(client, trace, "authenticated-success")
        screenshot(client, trace, screenshot_directory, "authenticated-success")

        generation = ready["generation"]
        client.click(client.find("#metis-reset"))
        _wait_for_text(client, "metis-status", "Mount reset; the previous fragment generation is stale", include=True, timeout_ms=timeout_ms)
        reset = _snapshot(client, trace, "reset-stale-generation")
        if reset["generation"] != generation + 1 or any(
            _text(reset, element_id) != "—"
            for element_id in ("metis-events", "http-response", "metis-negative")
        ):
            raise BrowserRuntimeError("fragment reset did not clear the stale generation")
        screenshot(client, trace, screenshot_directory, "reset-stale-generation")
        trace.actions.append({"action": "reset", "stale_state": "cleared", "generation": reset["generation"]})

        client.click(client.find("#metis-fragment"))
        _wait_for_text(client, "metis-status", FRAGMENT_SUCCESS, include=True, timeout_ms=timeout_ms)
        recovered = _snapshot(client, trace, "remounted-success")
        _assert_ready(
            recovered,
            expected_generation=reset["generation"],
            require_negative=False,
        )
        if browser_heap:
            browser_heap_sample(client, trace, "remounted-success")
        if browser_memory:
            browser_memory_sample(client, trace, "remounted-success")
        screenshot(client, trace, screenshot_directory, "remounted-success")
        trace.actions.append({"action": "remounted-fragment", "status": 200, "generation": recovered["generation"]})
        trace.cleanup = {
            "session_closed": False,
            "request_state": recovered["request_state"],
            "stale_generation": "unchanged",
            "negative_probes": "malformed 400; unauthorized 401; target rejected; stale unchanged",
            "mounted_page": "http-health.html",
        }
        return trace
    finally:
        client.close()
        if trace is not None:
            trace.cleanup["session_closed"] = True
