"""Run the Metis browser workbench through a W3C WebDriver endpoint.

The runner deliberately uses only the Python standard library.  Browser and
driver installation stays with the host or CI job; this file owns the bounded
action trace, semantic assertions, screenshots and teardown evidence shared by
Chromium, Firefox and WebKit.
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import subprocess
import urllib.parse
from typing import Any, Dict, List, Mapping, Optional

from browser_protocol import (
    ELEMENT_KEY,
    MAX_TRACE_BYTES,
    MAX_URL_BYTES,
    MAX_WAIT_MILLISECONDS,
    ROOT,
    BrowserRuntimeError,
    StaticServer,
    WebDriverClient,
    _bounded_text,
    _safe_path,
)
from browser_trace import BrowserEngine, Trace, UNSUPPORTED_NATIVE_OPERATIONS, screenshot


BRIDGE_MODES = ("disconnected", "authorized")


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
) -> Trace:
    """Execute the same input, bridge and teardown trace for every engine."""
    if bridge not in BRIDGE_MODES:
        raise BrowserRuntimeError(f"unsupported bridge mode {bridge!r}; choose {', '.join(BRIDGE_MODES)}")
    trace: Optional[Trace] = None
    stopped_snapshot: Optional[Dict[str, Any]] = None
    remounted_snapshot: Optional[Dict[str, Any]] = None
    try:
        client.create_session(engine.webdriver_name)
        client.set_timeouts(timeout_ms)
        trace = Trace(engine, url, bridge, revision, client.capabilities)
        client.navigate(url)
        _wait_for_selector(client, "#metis-form", timeout_ms=timeout_ms)
        initial = _snapshot(client, trace, "initial")
        screenshot(client, trace, screenshot_directory, "initial")
        if bridge == "authorized":
            _wait_for_text(client, "metis-status", "Authorized backend session ready", include=True, timeout_ms=timeout_ms)
            trace.actions.append({"action": "await-authorized-bridge", "result": "ready"})

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
            _assert_remount_has_no_result(remounted_snapshot)
            _assert_no_pending_request(remounted_snapshot)
            screenshot(client, trace, screenshot_directory, "remounted")
            trace.actions.append({"action": "stop-remount", "stale_result": False})

        if stopped_snapshot is None or remounted_snapshot is None:
            raise BrowserRuntimeError("browser lifecycle trace did not collect both teardown snapshots")
        trace.cleanup = {
            "session_closed": False,
            "stopped_message": True,
            "stopped_mounted_controls": stopped_snapshot["mounted_controls"],
            "remounted_mounted_controls": remounted_snapshot["mounted_controls"],
            "pending_requests": 0,
            "pending_request_observation": "remounted metis-form aria-busy=false",
            "listeners_or_tasks": "no stale completion after generation teardown; provider count unavailable",
        }
        return trace
    finally:
        client.close()
        if trace is not None:
            trace.cleanup["session_closed"] = True


def _assert_stopped(snapshot: Mapping[str, Any]) -> None:
    """Require the stopped root to contain no mounted application controls."""
    if snapshot.get("mounted_controls") != 0 or snapshot.get("app_text") != "Metis browser host stopped.":
        raise BrowserRuntimeError(f"stopped DOM retained application state: {snapshot}")


def _assert_remount_has_no_result(snapshot: Mapping[str, Any]) -> None:
    """Reject a response delivered to a new lifecycle generation."""
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


def _write_trace(path: pathlib.Path, document: Mapping[str, Any]) -> None:
    """Write one bounded, deterministic JSON trace."""
    _safe_path(path, directory=ROOT / "output")
    path.parent.mkdir(parents=True, exist_ok=True)
    encoded = json.dumps(document, indent=2, sort_keys=True).encode("utf-8")
    if len(encoded) > MAX_TRACE_BYTES:
        raise BrowserRuntimeError("browser trace exceeds the 512 KiB budget")
    path.write_bytes(encoded + b"\n")


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", required=True, choices=[engine.value for engine in BrowserEngine])
    parser.add_argument("--scenario", choices=("workbench", "canvas"), default="workbench")
    parser.add_argument("--driver-url", help="W3C WebDriver endpoint; defaults to METIS_WEBDRIVER_<ENGINE>_URL")
    parser.add_argument("--url", help="already-running browser workbench URL")
    parser.add_argument("--serve-dir", type=pathlib.Path, help="serve one generated output/browser directory on loopback")
    parser.add_argument("--canvas-id", action="append", default=[], help="canvas DOM id for the format-neutral trusted-input scenario; repeat per canvas")
    parser.add_argument("--consumer-revision", help="40-hex revision of the application consuming the format-neutral canvas seam")
    parser.add_argument("--bridge", choices=BRIDGE_MODES, default="disconnected")
    parser.add_argument("--cancel", action="store_true", help="submit a delayed authorized request, stop, remount and check stale-response disposal")
    parser.add_argument("--cancel-grace-ms", type=int, default=4_000)
    parser.add_argument("--timeout-seconds", type=float, default=30.0)
    parser.add_argument("--output", type=pathlib.Path, help="trace path; defaults to output/browser/runtime/<engine>-<scenario>.json")
    return parser.parse_args()


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
        if not 1 <= arguments.cancel_grace_ms <= MAX_WAIT_MILLISECONDS:
            raise BrowserRuntimeError(f"cancel-grace-ms must be between 1 and {MAX_WAIT_MILLISECONDS}")
        if arguments.cancel and arguments.bridge != "authorized":
            raise BrowserRuntimeError("--cancel requires --bridge authorized")
        run_canvas_scenario = None
        consumer_revision = None
        if arguments.scenario == "canvas":
            from browser_canvas import run_canvas_scenario, validate_canvas_ids, validate_consumer_revision

            if arguments.bridge != "disconnected":
                raise BrowserRuntimeError("canvas scenarios do not use the workbench bridge")
            if arguments.cancel:
                raise BrowserRuntimeError("--cancel is only valid for the workbench scenario")
            canvas_ids = validate_canvas_ids(arguments.canvas_id)
            consumer_revision = validate_consumer_revision(arguments.consumer_revision)
        elif arguments.canvas_id:
            raise BrowserRuntimeError("--canvas-id requires --scenario canvas")
        elif arguments.consumer_revision is not None:
            raise BrowserRuntimeError("--consumer-revision requires --scenario canvas")
        driver_url = arguments.driver_url or os.environ.get(f"METIS_WEBDRIVER_{engine.value.upper()}_URL")
        if not driver_url:
            raise BrowserRuntimeError(f"set --driver-url or METIS_WEBDRIVER_{engine.value.upper()}_URL")
        if arguments.url and arguments.serve_dir:
            raise BrowserRuntimeError("--url and --serve-dir are mutually exclusive")
        if not arguments.url and not arguments.serve_dir:
            raise BrowserRuntimeError("one of --url or --serve-dir is required")
        if arguments.bridge == "authorized" and arguments.serve_dir:
            raise BrowserRuntimeError("authorized runs require --url with host session configuration")
        revision = _revision()
        server = StaticServer(arguments.serve_dir) if arguments.serve_dir else None
        if server is not None:
            with server as origin:
                url = origin
                client = WebDriverClient(driver_url, arguments.timeout_seconds)
                if arguments.scenario == "canvas":
                    trace = run_canvas_scenario(client, engine, url, revision, output.parent / "screenshots" / engine.value / "canvas", timeout_ms, canvas_ids, consumer_revision)
                else:
                    trace = run_scenario(client, engine, url, arguments.bridge, revision, output.parent / "screenshots" / engine.value, timeout_ms, arguments.cancel, arguments.cancel_grace_ms)
        else:
            url = arguments.url
            if url is None:
                raise BrowserRuntimeError("browser URL was not provided")
            _validate_bridge_url(url, arguments.bridge)
            client = WebDriverClient(driver_url, arguments.timeout_seconds)
            if arguments.scenario == "canvas":
                trace = run_canvas_scenario(client, engine, url, revision, output.parent / "screenshots" / engine.value / "canvas", timeout_ms, canvas_ids, consumer_revision)
            else:
                trace = run_scenario(client, engine, url, arguments.bridge, revision, output.parent / "screenshots" / engine.value, timeout_ms, arguments.cancel, arguments.cancel_grace_ms)
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


if __name__ == "__main__":
    raise SystemExit(main())
