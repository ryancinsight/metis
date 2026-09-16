"""Bounded same-page lifecycle evidence for the file-backed browser gallery."""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys
import time
from collections.abc import Callable

from browser_canvas import KeyboardTraceKind, capture_canvas_trace, validate_canvas_attributes
from browser_protocol import BrowserRuntimeError, WebDriverClient
from browser_runtime import _wait_for_selector, _wait_for_text
from browser_trace import Trace, browser_heap_sample


MAX_LIFECYCLE_CYCLES = 8
LIFECYCLE_WARMUP_CYCLES = 2
LIFECYCLE_GROWTH_MINIMUM_CYCLES = 4
MAX_LIFECYCLE_SUITE_SECONDS = 300
MAX_LIFECYCLE_CLEANUP_SECONDS = 10
WASM_PAGE_BYTES = 65_536
MAX_WASM32_BYTES = 1 << 32
LIFECYCLE_PHASES = ("mounted", "transfer", "decoded", "cine", "stopped")

OBSERVE_TRANSFER = """
const zone = document.getElementById('drop-zone');
const input = document.getElementById('file-input');
if (!zone || !input) throw new Error('file transfer controls are not mounted');
if (window.metisTransferObserver && typeof window.metisTransferObserver.cleanup === 'function')
  window.metisTransferObserver.cleanup();
window.metisFileEvidence = [];
window.metisInputFiles = null;
window.metisSelectedFile = null;
const observe = (type, event, list) => {
  const count = list.length;
  const files = count > 512 ? [] : Array.from(list);
  const bytes = files.reduce((n, f) => n + f.size, 0);
  if (window.metisFileEvidence.length === 16) window.metisFileEvidence.shift();
  window.metisFileEvidence.push({type, trusted: event.isTrusted,
    files: count, bytes: count > 512 ? null : bytes});
  if (type !== 'drop' && type !== 'change') return;
  if (window.metisSelectedFile === null && count > 0) {
    window.metisSelectedFile = list.item ? list.item(0) : list[0];
  }
  if (count > 512 || bytes > 256 * 1024 * 1024 || files.some(f => f.size > 64 * 1024 * 1024)) {
    window.metisInputFiles = Promise.resolve({rejected: 'file evidence exceeds host bounds'});
    return;
  }
  const evidence = (async () => {
    const results = [];
    for (const file of files) {
      const hash = await crypto.subtle.digest('SHA-256', await file.arrayBuffer());
      results.push({name: file.name, bytes: file.size,
        sha256: Array.from(new Uint8Array(hash), b => b.toString(16).padStart(2, '0')).join('')});
    }
    return results;
  })();
  window.metisInputFiles = evidence.then(
    value => value,
    error => ({diagnostic: error && typeof error.name === 'string' ? error.name : 'Error'})
  );
};
const listeners = [];
for (const type of ['dragenter', 'dragover', 'drop']) {
  const listener = event => {
    observe(type, event, event.dataTransfer ? event.dataTransfer.files : []);
  };
  zone.addEventListener(type, listener, {capture: true});
  listeners.push([zone, type, listener]);
}
const changeListener = event => observe('change', event, input.files);
input.addEventListener('change', changeListener, {capture: true});
listeners.push([input, 'change', changeListener]);
window.metisTransferObserver = {
  cleanup() {
    for (const [target, type, listener] of listeners)
      target.removeEventListener(type, listener, {capture: true});
    listeners.length = 0;
    delete window.metisFileEvidence;
    delete window.metisInputFiles;
    delete window.metisSelectedFile;
    delete window.metisTransferObserver;
    return {listeners_removed: 4, globals_cleared: true};
  }
};
const r = zone.getBoundingClientRect();
return {x: r.x + r.width / 2, y: r.y + r.height / 2,
        visible: r.width > 0 && r.height > 0 && r.bottom <= innerHeight};
"""

CLEANUP_TRANSFER = """
if (window.metisTransferObserver && typeof window.metisTransferObserver.cleanup === 'function')
  return window.metisTransferObserver.cleanup();
delete window.metisFileEvidence;
delete window.metisInputFiles;
delete window.metisSelectedFile;
delete window.metisTransferObserver;
return {listeners_removed: 0, globals_cleared: true};
"""

MOUNT_GALLERY = """
const done = arguments[arguments.length - 1];
const gallery = window.metisGallery;
if (!gallery || typeof gallery.mount !== 'function' || typeof gallery.sample !== 'function') {
  done({ok: false, error: 'gallery lifecycle API unavailable'});
  return;
}
Promise.resolve(gallery.mount()).then(
  () => done({ok: true, sample: gallery.sample()}),
  error => done({ok: false, error: error && error.name ? error.name : 'Error'})
);
"""

SAMPLE_GALLERY = """
const gallery = window.metisGallery;
if (!gallery || typeof gallery.sample !== 'function')
  throw new Error('gallery lifecycle API unavailable');
return gallery.sample();
"""

STOP_GALLERY = """
const gallery = window.metisGallery;
if (!gallery || typeof gallery.stop !== 'function' || typeof gallery.sample !== 'function')
  throw new Error('gallery lifecycle API unavailable');
const result = gallery.stop();
if (result && typeof result.then === 'function')
  throw new Error('gallery stop must be synchronous');
return gallery.sample();
"""

STOPPED_CLEANUP = """
return {
  transfer_observer_cleared: !Object.prototype.hasOwnProperty.call(window, 'metisTransferObserver'),
  file_evidence_cleared: !Object.prototype.hasOwnProperty.call(window, 'metisFileEvidence'),
  input_files_cleared: !Object.prototype.hasOwnProperty.call(window, 'metisInputFiles'),
  selected_file_cleared: !Object.prototype.hasOwnProperty.call(window, 'metisSelectedFile'),
  canvas_trace_cleared: !Object.prototype.hasOwnProperty.call(window, '__metisCanvasTraceState'),
  file_input_removed: document.getElementById('file-input') === null,
  drop_zone_removed: document.getElementById('drop-zone') === null
};
"""


def validate_lifecycle_request(args: argparse.Namespace) -> int:
    """Validate the bounded repeated-gallery mode and return its cycle count."""
    cycles = getattr(args, "lifecycle_cycles", 1)
    if type(cycles) is not int or not 1 <= cycles <= MAX_LIFECYCLE_CYCLES:
        raise BrowserRuntimeError(
            f"lifecycle-cycles must be between 1 and {MAX_LIFECYCLE_CYCLES}"
        )
    if cycles > 1:
        if cycles < LIFECYCLE_GROWTH_MINIMUM_CYCLES:
            raise BrowserRuntimeError(
                f"repeated gallery lifecycle requires at least "
                f"{LIFECYCLE_GROWTH_MINIMUM_CYCLES} cycles for the growth gate"
            )
        if args.input not in ("chooser", "chromium"):
            raise BrowserRuntimeError("repeated gallery lifecycle requires chooser or chromium input")
        if args.canvas_trace is None or args.keyboard_trace != "cine-rate":
            raise BrowserRuntimeError(
                "repeated gallery lifecycle requires --canvas-trace and --keyboard-trace cine-rate"
            )
        suite_seconds = getattr(args, "lifecycle_timeout_seconds", MAX_LIFECYCLE_SUITE_SECONDS)
        if type(suite_seconds) is not int or not 1 <= suite_seconds <= MAX_LIFECYCLE_SUITE_SECONDS:
            raise BrowserRuntimeError(
                f"lifecycle-timeout-seconds must be between 1 and "
                f"{MAX_LIFECYCLE_SUITE_SECONDS}"
            )
    return cycles


class _DeadlineClient:
    """Apply one monotonic suite deadline to every delegated browser command."""

    def __init__(self, client: WebDriverClient, deadline: float) -> None:
        self._client = client
        self._deadline = deadline
        self._transport_timeout = client._timeout

    def remaining_milliseconds(self) -> int:
        remaining = self._deadline - time.monotonic()
        if remaining <= 0:
            raise BrowserRuntimeError("gallery lifecycle suite deadline exceeded")
        self._client._timeout = min(self._transport_timeout, remaining)
        return max(1, int(remaining * 1000))

    def restore(self) -> None:
        self._client._timeout = self._transport_timeout

    def __getattr__(self, name: str):
        value = getattr(self._client, name)
        if not callable(value):
            return value

        def bounded(*args, **kwargs):
            self.remaining_milliseconds()
            result = value(*args, **kwargs)
            self.remaining_milliseconds()
            return result

        return bounded


def _validate_gallery_sample(value: object, *, mounted: bool) -> dict:
    """Validate one lifecycle sample without interpreting consumer semantics."""
    if not isinstance(value, dict):
        raise BrowserRuntimeError("gallery lifecycle sample returned a non-object")
    sample = {}
    wasm_bytes = value.get("wasm_bytes")
    if (
        type(wasm_bytes) is not int
        or not WASM_PAGE_BYTES <= wasm_bytes <= MAX_WASM32_BYTES
        or wasm_bytes % WASM_PAGE_BYTES != 0
    ):
        raise BrowserRuntimeError("gallery lifecycle sample returned invalid wasm_bytes")
    sample["wasm_bytes"] = wasm_bytes
    for name in ("host_listeners", "consumer_listeners"):
        item = value.get(name)
        if type(item) is not int or not 0 <= item <= (1 << 40):
            raise BrowserRuntimeError(f"gallery lifecycle sample returned invalid {name}")
        sample[name] = item
    if type(value.get("mounted")) is not bool:
        raise BrowserRuntimeError("gallery lifecycle sample returned invalid mounted state")
    sample["mounted"] = value["mounted"]
    if sample["mounted"] is not mounted:
        raise BrowserRuntimeError("gallery lifecycle sample returned the wrong mounted state")
    if mounted and (sample["host_listeners"] == 0 or sample["consumer_listeners"] == 0):
        raise BrowserRuntimeError("mounted gallery reported no listener guards")
    if not mounted and (sample["host_listeners"] != 0 or sample["consumer_listeners"] != 0):
        raise BrowserRuntimeError("stopped gallery retained listener guards")
    return sample


def _lifecycle_phase(
    client: WebDriverClient,
    trace: Trace,
    cycle: int,
    phase: str,
    sample: object,
    *,
    mounted: bool,
) -> dict:
    """Record one listener, WASM-capacity, and optional JavaScript-heap sample."""
    gallery = _validate_gallery_sample(sample, mounted=mounted)
    heap = browser_heap_sample(client, trace, f"cycle-{cycle}-{phase}")
    return {"phase": phase, "gallery": gallery, "browser_heap": heap}


def assert_lifecycle_growth(records: object) -> None:
    """Require stable guards and post-warmup WASM capacity across real cycles."""
    if not isinstance(records, list) or len(records) < LIFECYCLE_GROWTH_MINIMUM_CYCLES:
        raise BrowserRuntimeError(
            f"gallery growth regression requires at least {LIFECYCLE_GROWTH_MINIMUM_CYCLES} cycles"
        )
    reference_phases = records[0].get("phases") if isinstance(records[0], dict) else None
    if not isinstance(reference_phases, list):
        raise BrowserRuntimeError("gallery lifecycle records are malformed")
    if any(not isinstance(phase, dict) for phase in reference_phases):
        raise BrowserRuntimeError("gallery lifecycle phase sample is malformed")
    reference_names = [phase.get("phase") for phase in reference_phases]
    if reference_names != list(LIFECYCLE_PHASES):
        raise BrowserRuntimeError("gallery lifecycle record omitted or reordered required phases")
    live_listener_counts = {
        phase["phase"]: (
            phase["gallery"]["host_listeners"],
            phase["gallery"]["consumer_listeners"],
        )
        for phase in reference_phases
        if phase.get("phase") != "stopped"
    }
    for expected_cycle, record in enumerate(records, start=1):
        if (
            not isinstance(record, dict)
            or record.get("cycle") != expected_cycle
            or record.get("status") != "complete"
        ):
            raise BrowserRuntimeError("gallery lifecycle records are not sequential and complete")
        phases = record.get("phases") if isinstance(record, dict) else None
        if not isinstance(phases, list) or any(not isinstance(phase, dict) for phase in phases):
            raise BrowserRuntimeError("gallery lifecycle phase sample is malformed")
        if [phase.get("phase") for phase in phases] != reference_names:
            raise BrowserRuntimeError("gallery lifecycle phase sequence changed between cycles")
        for phase in phases:
            gallery = phase.get("gallery")
            if not isinstance(gallery, dict):
                raise BrowserRuntimeError("gallery lifecycle phase sample is malformed")
            if phase["phase"] == "stopped":
                _validate_gallery_sample(gallery, mounted=False)
            else:
                _validate_gallery_sample(gallery, mounted=True)
                listeners = (gallery["host_listeners"], gallery["consumer_listeners"])
                if listeners != live_listener_counts[phase["phase"]]:
                    raise BrowserRuntimeError(
                        f"gallery listener guards grew during {phase['phase']}"
                    )
    post_warmup = records[LIFECYCLE_WARMUP_CYCLES:]
    capacity_profile = [phase["gallery"]["wasm_bytes"] for phase in post_warmup[0]["phases"]]
    for record in post_warmup[1:]:
        observed = [phase["gallery"]["wasm_bytes"] for phase in record["phases"]]
        if observed != capacity_profile:
            raise BrowserRuntimeError("gallery WebAssembly capacity grew after warmup")


def _mount_gallery(client: WebDriverClient) -> dict:
    result = client.execute_async(MOUNT_GALLERY)
    if not isinstance(result, dict) or result.get("ok") is not True:
        detail = result.get("error") if isinstance(result, dict) else result
        raise BrowserRuntimeError(f"gallery mount failed: {detail!r}")
    return _validate_gallery_sample(result.get("sample"), mounted=True)


def _stop_gallery(client: WebDriverClient) -> dict:
    return _validate_gallery_sample(client.execute(STOP_GALLERY), mounted=False)


def _stopped_cleanup(client: WebDriverClient) -> dict:
    value = client.execute(STOPPED_CLEANUP)
    expected = {
        "transfer_observer_cleared": True,
        "file_evidence_cleared": True,
        "input_files_cleared": True,
        "selected_file_cleared": True,
        "canvas_trace_cleared": True,
        "file_input_removed": True,
        "drop_zone_removed": True,
    }
    if value != expected:
        raise BrowserRuntimeError(f"stopped gallery retained browser resources: {value!r}")
    return value


def run_gallery_lifecycle(
    client: WebDriverClient,
    trace: Trace,
    canvas_trace_factory: Callable[[], Trace],
    files: list[pathlib.Path],
    total: int,
    expected_files: list[dict],
    oracle: dict,
    canvas_ids: list[str],
    canvas_attributes: list[str],
    input_source: str,
    cycles: int,
    screenshot_directory: pathlib.Path,
    transfer_files: Callable[[WebDriverClient, list[pathlib.Path], dict], None],
    observe_transfer_script: str,
    cleanup_transfer: Callable[[WebDriverClient], dict],
    canvas_pixels_script: str,
    canvas_traces: list[Trace],
    suite_timeout_seconds: int = MAX_LIFECYCLE_SUITE_SECONDS,
) -> list[Trace]:
    """Run bounded same-page mount, transfer, decode, cine, and stop cycles."""
    if type(suite_timeout_seconds) is not int or not 1 <= suite_timeout_seconds <= MAX_LIFECYCLE_SUITE_SECONDS:
        raise BrowserRuntimeError(
            f"lifecycle-timeout-seconds must be between 1 and {MAX_LIFECYCLE_SUITE_SECONDS}"
        )
    suite_deadline = time.monotonic() + suite_timeout_seconds
    cleanup_reserve = min(MAX_LIFECYCLE_CLEANUP_SECONDS, suite_timeout_seconds / 5)
    bounded = _DeadlineClient(client, suite_deadline - cleanup_reserve)
    cleanup_bounded = _DeadlineClient(client, suite_deadline)
    records = []
    event_type = "change" if input_source == "chooser" else "drop"
    expected_event = {
        "type": event_type,
        "trusted": True,
        "files": len(files),
        "bytes": total,
    }
    wanted_transfer = f"Byte access: read {total} bytes from {len(files)} file(s)"
    file_manifest_sha256 = hashlib.sha256(
        json.dumps(expected_files, sort_keys=True).encode()
    ).hexdigest()
    trace.metrics["gallery_lifecycle"] = records
    try:
        for cycle in range(1, cycles + 1):
            phases = []
            record = {"cycle": cycle, "status": "running", "phases": phases}
            records.append(record)
            observer_installed = False
            stopped = False
            try:
                mounted = _mount_gallery(bounded)
                _wait_for_text(
                    bounded,
                    "gallery-status",
                    "Ready.",
                    timeout_ms=min(30_000, bounded.remaining_milliseconds()),
                    include=True,
                )
                phases.append(
                    _lifecycle_phase(bounded, trace, cycle, "mounted", mounted, mounted=True)
                )
                point = bounded.execute(observe_transfer_script)
                observer_installed = True
                if not isinstance(point, dict) or point.get("visible") is not True:
                    raise BrowserRuntimeError("file drop zone is not visible inside the viewport")
                transfer_files(bounded, files, point)
                _wait_for_selector(
                    bounded,
                    '#drop-zone:is([data-byte-state="complete"],[data-byte-state="failed"])',
                    timeout_ms=min(60_000, bounded.remaining_milliseconds()),
                )
                transfer = bounded.execute(
                    "return {status:document.getElementById('drop-status').textContent,"
                    "bytes:document.getElementById('drop-byte-status').textContent};"
                )
                if transfer.get("bytes") != wanted_transfer:
                    raise BrowserRuntimeError(f"file transfer did not complete: {transfer}")
                events = bounded.execute("return window.metisFileEvidence;")
                if not isinstance(events, list):
                    raise BrowserRuntimeError("file transfer evidence is malformed")
                selected = [event for event in events if event.get("type") == event_type]
                if selected != [expected_event]:
                    raise BrowserRuntimeError(
                        f"cycle {cycle} {event_type} event differs from the real-file contract: {selected}"
                    )
                identities = bounded.execute_async(
                    "const done=arguments[arguments.length-1];"
                    "Promise.resolve(window.metisInputFiles).then(done,"
                    "e=>done({diagnostic:String(e)}));"
                )
                if (
                    not isinstance(identities, list)
                    or sorted(identities, key=lambda file: file["name"]) != expected_files
                ):
                    raise BrowserRuntimeError(
                        f"cycle {cycle} browser file content differs from the selected fixture"
                    )
                trace.actions.append({"cycle": cycle, "input": input_source, "events": events})
                trace.snapshots.append(
                    {
                        "cycle": cycle,
                        "transfer": transfer,
                        "file_manifest_sha256": file_manifest_sha256,
                    }
                )
                phases.append(
                    _lifecycle_phase(
                        bounded,
                        trace,
                        cycle,
                        "transfer",
                        bounded.execute(SAMPLE_GALLERY),
                        mounted=True,
                    )
                )
                for canvas_id in canvas_ids:
                    expected = oracle[canvas_id]
                    attributes = expected.get("attributes", {})
                    validate_canvas_attributes(list(attributes))
                    for name, value in attributes.items():
                        if not isinstance(value, str) or not value.isalnum():
                            raise BrowserRuntimeError("oracle attribute values must be alphanumeric")
                        _wait_for_selector(
                            bounded,
                            f'#{canvas_id}[{name}="{value}"]',
                            timeout_ms=min(60_000, bounded.remaining_milliseconds()),
                        )
                    actual = bounded.execute_async(canvas_pixels_script, [canvas_id])
                    wanted = {
                        key: expected[key]
                        for key in ("width", "height", "non_black_pixels", "rgba_sha256")
                    }
                    if actual != wanted:
                        raise BrowserRuntimeError(
                            f"cycle {cycle} canvas {canvas_id}: expected {wanted}, found {actual}"
                        )
                    trace.snapshots.append({"cycle": cycle, "id": canvas_id, **actual})
                phases.append(
                    _lifecycle_phase(
                        bounded,
                        trace,
                        cycle,
                        "decoded",
                        bounded.execute(SAMPLE_GALLERY),
                        mounted=True,
                    )
                )
                canvas_trace = canvas_trace_factory()
                canvas_traces.append(canvas_trace)
                capture_canvas_trace(
                    bounded,
                    canvas_trace,
                    screenshot_directory / f"cycle-{cycle}",
                    canvas_ids,
                    canvas_attributes,
                    frame_timeout_ms=min(4_000, bounded.remaining_milliseconds()),
                    keyboard_trace=KeyboardTraceKind.CINE_RATE,
                )
                phases.append(
                    _lifecycle_phase(
                        bounded,
                        trace,
                        cycle,
                        "cine",
                        bounded.execute(SAMPLE_GALLERY),
                        mounted=True,
                    )
                )
                cleanup = cleanup_transfer(bounded)
                observer_installed = False
                stopped_sample = _stop_gallery(bounded)
                stopped = True
                record["stopped_cleanup"] = _stopped_cleanup(bounded)
                phases.append(
                    _lifecycle_phase(
                        bounded,
                        trace,
                        cycle,
                        "stopped",
                        stopped_sample,
                        mounted=False,
                    )
                )
                record["observer_cleanup"] = cleanup
                record["status"] = "complete"
            finally:
                primary_error = sys.exc_info()[1]
                cleanup_errors = []
                if observer_installed:
                    try:
                        record["observer_cleanup"] = cleanup_transfer(cleanup_bounded)
                    except BrowserRuntimeError as error:
                        cleanup_errors.append(("file transfer observer", error))
                if not stopped:
                    try:
                        stopped_sample = _stop_gallery(cleanup_bounded)
                        record["cleanup_stop"] = stopped_sample
                        record["cleanup_state"] = _stopped_cleanup(cleanup_bounded)
                    except BrowserRuntimeError as error:
                        cleanup_errors.append(("gallery stop", error))
                if primary_error is not None:
                    record["status"] = "failed"
                    record["error"] = type(primary_error).__name__
                if cleanup_errors:
                    record["cleanup_errors"] = [operation for operation, _ in cleanup_errors]
                    if primary_error is not None:
                        for operation, error in cleanup_errors:
                            primary_error.add_note(f"{operation} cleanup also failed: {error}")
                    else:
                        operation, error = cleanup_errors[0]
                        error.add_note(f"failed cleanup operation: {operation}")
                        raise error
    finally:
        cleanup_bounded.restore()
        bounded.restore()
    assert_lifecycle_growth(records)
    trace.cleanup.update(
        {
            "lifecycle_cycles": len(records),
            "warmup_cycles": LIFECYCLE_WARMUP_CYCLES,
            "growth_gate": "stable per-phase WebAssembly capacity after warmup",
            "stopped_listener_guards": 0,
            "transfer_observer_listeners": 0,
        }
    )
    return canvas_traces
