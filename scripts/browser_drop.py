"""Prove file-backed browser input and capture consumer-owned canvas output.

Uses the existing bounded WebDriver transport. The chooser mode selects actual
local files through the standard W3C file-input command on every admitted
engine; the chromium mode uses ``Input.dispatchDragEvent`` for the explicit
Chromium drag path. Neither path creates page-owned ``File`` objects.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile

from browser_protocol import (
    ROOT, BrowserRuntimeError, StaticServer, WebDriverClient, _safe_path, parse_device_scale,
)
from browser_canvas import (
    capture_canvas_trace,
    validate_canvas_ids, validate_canvas_attributes, validate_consumer_revision,
    _element_screenshot,
)
from browser_runtime import _wait_for_text, _wait_for_selector, _write_trace
from browser_trace import BrowserEngine, Trace, record_device_scale, screenshot


# Independent test oracle for the existing host admission contract.
MAX_FILES = 512
MAX_FILE_BYTES = 64 * 1024 * 1024
MAX_BATCH_BYTES = 256 * 1024 * 1024

OBSERVE_TRANSFER = """
const zone = document.getElementById('drop-zone');
const input = document.getElementById('file-input');
if (!zone || !input) throw new Error('file transfer controls are not mounted');
window.metisFileEvidence = [];
window.metisInputFiles = null;
const observe = (type, event, list) => {
  const count = list.length;
  const files = count > 512 ? [] : Array.from(list);
  const bytes = files.reduce((n, f) => n + f.size, 0);
  if (window.metisFileEvidence.length === 16) window.metisFileEvidence.shift();
  window.metisFileEvidence.push({type, trusted: event.isTrusted,
    files: count, bytes: count > 512 ? null : bytes});
  if (type !== 'drop' && type !== 'change') return;
  if (count > 512 || bytes > 256 * 1024 * 1024 || files.some(f => f.size > 64 * 1024 * 1024)) {
    window.metisInputFiles = Promise.resolve({rejected: 'file evidence exceeds host bounds'});
    return;
  }
  window.metisInputFiles = (async () => {
    const results = [];
    for (const file of files) {
      const hash = await crypto.subtle.digest('SHA-256', await file.arrayBuffer());
      results.push({name: file.name, bytes: file.size,
        sha256: Array.from(new Uint8Array(hash), b => b.toString(16).padStart(2, '0')).join('')});
    }
    return results;
  })();
};
for (const type of ['dragenter', 'dragover', 'drop']) {
  zone.addEventListener(type, event => {
    observe(type, event, event.dataTransfer ? event.dataTransfer.files : []);
  }, {capture: true});
}
input.addEventListener('change', event => observe('change', event, input.files), {capture: true});
const r = zone.getBoundingClientRect();
return {x: r.x + r.width / 2, y: r.y + r.height / 2,
        visible: r.width > 0 && r.height > 0 && r.bottom <= innerHeight};
"""

CANVAS_PIXELS = """
const done = arguments[arguments.length - 1];
const canvas = document.getElementById(arguments[0]);
if (!(canvas instanceof HTMLCanvasElement) || canvas.width > 4096 || canvas.height > 4096)
  throw new Error('canvas is missing or exceeds the capture bound');
const pixels = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data;
let nonBlack = 0;
for (let i = 0; i < pixels.length; i += 4)
  if (pixels[i] || pixels[i + 1] || pixels[i + 2]) ++nonBlack;
crypto.subtle.digest('SHA-256', pixels).then(hash => done({
  width: canvas.width, height: canvas.height, non_black_pixels: nonBlack,
  rgba_sha256: Array.from(new Uint8Array(hash), b => b.toString(16).padStart(2, '0')).join('')
}), error => done({diagnostic: String(error)}));
"""


def study_files(directory: pathlib.Path, pattern: str = "*") -> tuple[list[pathlib.Path], int]:
    """Select immediate regular files without traversing a folder or link."""
    files = []
    total = 0
    if "/" in pattern or "\\" in pattern or pattern in ("", ".", ".."):
        raise BrowserRuntimeError("file pattern must select immediate filenames")
    for path in directory.glob(pattern):
        if path.is_symlink():
            raise BrowserRuntimeError("file drop fixture contains a symbolic link")
        if not path.is_file():
            raise BrowserRuntimeError("file drop fixture must contain files only")
        size = path.stat().st_size
        if len(files) == MAX_FILES or size > MAX_FILE_BYTES or total + size > MAX_BATCH_BYTES:
            raise BrowserRuntimeError("file drop fixture exceeds host count or byte bounds")
        files.append(path.resolve())
        total += size
    if not files:
        raise BrowserRuntimeError("file drop fixture is empty")
    return sorted(files), total


def dispatch_files(client: WebDriverClient, files: list[pathlib.Path], point: dict) -> None:
    """Use Chromium's file-backed input path, explicitly outside W3C actions."""
    name = client.capabilities.get("browserName")
    if name not in ("chrome", "MicrosoftEdge", "msedge"):
        raise BrowserRuntimeError("file-backed protocol drops require Chromium")
    endpoint = "ms/cdp/execute" if name in ("MicrosoftEdge", "msedge") else "goog/cdp/execute"
    data = {"items": [], "files": [str(path) for path in files], "dragOperationsMask": 1}
    for event in ("dragEnter", "dragOver", "drop"):
        client._request("POST", client._session_path(endpoint), {
            "cmd": "Input.dispatchDragEvent",
            "params": {"type": event, "x": point["x"], "y": point["y"], "data": data},
        })


def resolve_browser_target(
    engine_name: str | None,
    browser_name: str | None,
) -> tuple[BrowserEngine, str]:
    """Resolve one matrix engine and its admitted W3C browser name."""
    if engine_name is None:
        inferred = {
            None: BrowserEngine.CHROMIUM,
            "chrome": BrowserEngine.CHROMIUM,
            "MicrosoftEdge": BrowserEngine.CHROMIUM,
            "firefox": BrowserEngine.FIREFOX,
            "safari": BrowserEngine.WEBKIT,
        }.get(browser_name)
        if inferred is None:
            raise BrowserRuntimeError(f"browser name {browser_name!r} is not in the conformance matrix")
        engine = inferred
    else:
        engine = BrowserEngine.parse(engine_name)
    return engine, engine.resolve_webdriver_name(browser_name)


def select_files(client: WebDriverClient, files: list[pathlib.Path]) -> None:
    """Use the standard W3C file chooser path for every admitted engine."""
    client.send_file_paths(client.find("#file-input"), files)


def check_rejections(
    client: WebDriverClient,
    trace: Trace,
    point: dict,
    input_source: str,
) -> None:
    """Exercise host admission and observer preflight with real sparse files."""
    with tempfile.TemporaryDirectory(prefix="metis-drop-") as directory:
        root = pathlib.Path(directory)
        cases = (("count", MAX_FILES + 1, 0),
                 ("file-bytes", 1, MAX_FILE_BYTES + 1),
                 ("batch-bytes", MAX_BATCH_BYTES // MAX_FILE_BYTES + 1, MAX_FILE_BYTES))
        for name, count, size in cases:
            folder = root / name
            folder.mkdir()
            files = []
            for index in range(count):
                path = folder / str(index)
                with path.open("wb") as stream:
                    stream.truncate(size)
                files.append(path)
            if input_source == "chromium":
                dispatch_files(client, files, point)
            elif input_source == "chooser":
                select_files(client, files)
            else:
                raise BrowserRuntimeError(f"rejection probes require an automated input source: {input_source}")
            _wait_for_selector(client, '#drop-zone[data-drop-state="rejected"][data-byte-state="failed"]', timeout_ms=60_000)
            observed = client.execute_async("const done=arguments[arguments.length-1]; Promise.resolve(window.metisInputFiles).then(done, e=>done({diagnostic:String(e)}));")
            if observed != {"rejected": "file evidence exceeds host bounds"}:
                raise BrowserRuntimeError(f"{name} evidence reader did not reject before reading: {observed}")
            trace.actions.append({"rejected": name, "files": count, "bytes": count * size,
                                  "input": input_source, "observer": observed})


def run(args: argparse.Namespace) -> dict:
    engine, browser_name = resolve_browser_target(
        getattr(args, "engine", None),
        getattr(args, "browser_name", None),
    )
    if args.input == "chromium" and engine is not BrowserEngine.CHROMIUM:
        raise BrowserRuntimeError("the chromium input source requires the Chromium engine")
    device_scale_milli = (
        parse_device_scale(args.device_scale)
        if getattr(args, "device_scale", None) is not None
        else None
    )
    files, total = study_files(args.files, args.pattern)
    expected_files = []
    for path in files:
        with path.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        expected_files.append({"name": path.name, "bytes": path.stat().st_size, "sha256": digest})
    validate_consumer_revision(args.consumer_revision)
    oracle = json.loads(args.oracle.read_text(encoding="utf-8"))
    ids = validate_canvas_ids(list(oracle))
    canvas_attributes = validate_canvas_attributes(args.canvas_attribute)
    canvas_trace_path = None
    if args.canvas_trace is not None:
        canvas_trace_path = _safe_path(args.canvas_trace.resolve(), directory=ROOT / "output")
    output = _safe_path(args.output.resolve(), directory=ROOT / "output")
    output.mkdir(parents=True, exist_ok=True)
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True, timeout=30).strip()
    client = WebDriverClient(args.driver_url, 120)
    canvas_trace: Trace | None = None
    trace = Trace(engine, "", "disconnected", revision, {}, args.consumer_revision)
    document = {"status": "failed"}
    try:
        with StaticServer(ROOT / "output" / "browser") as origin:
            trace.url = origin + "gallery.html"
            client.create_session(browser_name, device_scale_milli)
            trace.capabilities = {key: client.capabilities.get(key) for key in ("browserName", "browserVersion", "platformName")}
            client.set_timeouts(120_000)
            client._request("POST", client._session_path("window/rect"), {"width": 1440, "height": 1100})
            client.navigate(trace.url)
            _wait_for_text(client, "gallery-status", "Ready.", timeout_ms=30_000, include=True)
            record_device_scale(client, trace, device_scale_milli)
            point = client.execute(OBSERVE_TRANSFER)
            if not point["visible"]:
                raise BrowserRuntimeError("file drop zone is not visible inside the viewport")
            screenshot(client, trace, output, "before-drop")
            print(f"Drop {len(files)} files ({total} bytes) onto the visible file drop area.", flush=True)
            if args.input == "chromium":
                dispatch_files(client, files, point)
            elif args.input == "chooser":
                select_files(client, files)
            _wait_for_selector(client, '#drop-zone:is([data-byte-state="complete"],[data-byte-state="failed"])', timeout_ms=60_000)
            transfer = client.execute("return {status:document.getElementById('drop-status').textContent, bytes:document.getElementById('drop-byte-status').textContent};")
            trace.snapshots.append({"transfer": transfer})
            wanted_transfer = f"Byte access: read {total} bytes from {len(files)} file(s)"
            if transfer["bytes"] != wanted_transfer:
                raise BrowserRuntimeError(f"file transfer did not complete: {transfer}")
            events = client.execute("return window.metisFileEvidence;")
            event_type = "change" if args.input == "chooser" else "drop"
            selected = [event for event in events if event["type"] == event_type]
            expected_event = {"type": event_type, "trusted": True, "files": len(files), "bytes": total}
            if selected != [expected_event]:
                raise BrowserRuntimeError(f"{event_type} event differs from the real-file contract: {selected}")
            trace.actions.append({"input": args.input, "events": events})
            identities = client.execute_async("const done=arguments[arguments.length-1]; Promise.resolve(window.metisInputFiles).then(done, e=>done({diagnostic:String(e)}));")
            if not isinstance(identities, list) or sorted(identities, key=lambda file: file["name"]) != expected_files:
                raise BrowserRuntimeError("browser file content differs from the selected fixture")
            for canvas_id in ids:
                expected = oracle[canvas_id]
                # Consumer-provided DOM predicates remain opaque to the host runner.
                attributes = expected.get("attributes", {})
                validate_canvas_attributes(list(attributes))
                for name, value in attributes.items():
                    if not isinstance(value, str) or not value.isalnum():
                        raise BrowserRuntimeError("oracle attribute values must be alphanumeric")
                    selector = f'#{canvas_id}[{name}="{value}"]'
                    _wait_for_selector(client, selector, timeout_ms=60_000)
                actual = client.execute_async(CANVAS_PIXELS, [canvas_id])
                wanted = {key: expected[key] for key in ("width", "height", "non_black_pixels", "rgba_sha256")}
                if actual != wanted:
                    raise BrowserRuntimeError(f"canvas {canvas_id}: expected {wanted}, found {actual}")
                trace.snapshots.append({"id": canvas_id, **actual})
                _element_screenshot(client, trace, output, canvas_id, client.find("#" + canvas_id))
            viewport = client.execute("window.scrollTo(0,0); return {width:innerWidth,height:innerHeight,device_scale:devicePixelRatio};")
            screenshot(client, trace, output, "gallery")
            if canvas_trace_path is not None:
                canvas_trace = Trace(
                    engine,
                    trace.url,
                    "canvas",
                    revision,
                    client.capabilities,
                    args.consumer_revision,
                )
                record_device_scale(client, canvas_trace, device_scale_milli)
                capture_canvas_trace(
                    client,
                    canvas_trace,
                    canvas_trace_path.parent / "screenshots" / engine.value / "canvas",
                    ids,
                    canvas_attributes,
                )
            if args.input in ("chromium", "chooser"):
                expected_rgba = {canvas_id: oracle[canvas_id]["rgba_sha256"] for canvas_id in ids}
                if canvas_trace is not None:
                    expected_rgba = {
                        canvas_id: client.execute_async(CANVAS_PIXELS, [canvas_id])["rgba_sha256"]
                        for canvas_id in ids
                    }
                check_rejections(client, trace, point, args.input)
                for canvas_id in ids:
                    actual = client.execute_async(CANVAS_PIXELS, [canvas_id])
                    if actual["rgba_sha256"] != expected_rgba[canvas_id]:
                        raise BrowserRuntimeError("a rejected file batch changed the consumer frame")
            document = trace.document()
            document["input_source"] = args.input
            document["files"] = len(files)
            document["bytes"] = total
            document["viewport"] = viewport
            document["file_manifest_sha256"] = hashlib.sha256(json.dumps(expected_files, sort_keys=True).encode()).hexdigest()
            document["oracle_sha256"] = hashlib.sha256(args.oracle.read_bytes()).hexdigest()
            document["assets"] = {
                name: hashlib.sha256((ROOT / "output" / "browser" / name).read_bytes()).hexdigest()
                for name in ("gallery.html", "gallery.js", "gallery.css", "consumer/ritk_snap.js", "consumer/ritk_snap_bg.wasm")
            }
            document["sources"] = {
                name: hashlib.sha256((ROOT / "scripts" / name).read_bytes()).hexdigest()
                for name in ("browser.py", "browser_drop.py", "browser_protocol.py")
            }
    except (BrowserRuntimeError, OSError, ValueError) as error:
        document = trace.document("failed")
        document["error"] = str(error)
        if client.session_id is not None:
            try:
                screenshot(client, trace, output, "failure")
                document["diagnostic"] = client.execute("return {text:document.body.innerText.slice(0,8192),events:window.metisFileEvidence};")
            except BrowserRuntimeError as capture_error:
                document["capture_error"] = str(capture_error)
        raise
    finally:
        primary_error = sys.exc_info()[1]
        closed = False
        try:
            client.close()
            closed = True
            document["cleanup"] = {"session_closed": True}
        except BrowserRuntimeError as cleanup_error:
            document["status"] = "failed"
            document["cleanup"] = {"session_closed": False, "error": str(cleanup_error)}
            if primary_error is None:
                raise
        finally:
            (output / "trace.json").write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
            if canvas_trace is not None and canvas_trace_path is not None:
                canvas_trace.cleanup["session_closed"] = closed
                _write_trace(canvas_trace_path, canvas_trace.document())
    return document


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--driver-url", required=True)
    parser.add_argument("--engine", choices=tuple(engine.value for engine in BrowserEngine),
                        help="conformance engine; inferred from --browser-name when omitted")
    parser.add_argument("--browser-name", choices=("chrome", "MicrosoftEdge", "firefox", "safari"),
                        help="W3C browserName override within the selected engine family")
    parser.add_argument("--device-scale", help="requested browser device scale between 0.5 and 4, in decimal form")
    parser.add_argument("--files", type=pathlib.Path, required=True)
    parser.add_argument("--pattern", default="*", help="Immediate filename glob; no directory traversal")
    parser.add_argument("--oracle", type=pathlib.Path, required=True,
                        help="Consumer JSON mapping canvas IDs to exact dimensions, non-black pixel counts and attributes")
    parser.add_argument("--consumer-revision", required=True)
    parser.add_argument("--canvas-trace", type=pathlib.Path,
                        help="write a paired trusted canvas trace after the file drop")
    parser.add_argument("--canvas-attribute", action="append", default=[],
                        help="consumer-selected data-* attribute for the paired canvas trace")
    parser.add_argument("--input", choices=("manual", "chooser", "chromium"), default="manual",
                        help="manual OS drop, standard W3C chooser, or Chromium CDP drag")
    parser.add_argument("--output", type=pathlib.Path, default=ROOT / "output" / "browser" / "drop")
    run(parser.parse_args())


if __name__ == "__main__":
    main()
