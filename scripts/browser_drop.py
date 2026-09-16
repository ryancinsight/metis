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
import re
import subprocess
import sys
import tempfile
import urllib.parse

from browser_protocol import (
    ROOT, BrowserRuntimeError, StaticServer, WebDriverClient, _safe_path, parse_device_scale,
)
from browser_canvas import (
    capture_canvas_trace,
    KeyboardTraceKind,
    validate_canvas_ids, validate_canvas_attributes, validate_consumer_revision,
    _element_screenshot,
)
from browser_file_read import capture_file_read_diagnostic, capture_file_selection_diagnostic
from browser_runtime import _wait_for_text, _wait_for_selector, _write_trace
from browser_trace import BrowserEngine, Trace, browser_memory_sample, record_device_scale, screenshot
from browser_drop_lifecycle import (
    CLEANUP_TRANSFER,
    OBSERVE_TRANSFER,
    run_gallery_lifecycle,
    validate_lifecycle_request,
)


# Independent test oracle for the existing host admission contract.
MAX_FILES = 512
MAX_FILE_BYTES = 64 * 1024 * 1024
MAX_BATCH_BYTES = 256 * 1024 * 1024
MAX_PAGE_QUERY_PARAMETERS = 8
MAX_PAGE_QUERY_KEY_BYTES = 64
MAX_PAGE_QUERY_VALUE_BYTES = 128
MAX_PAGE_QUERY_BYTES = 1024
PAGE_QUERY_KEY_PATTERN = re.compile(r"[A-Za-z][A-Za-z0-9._~-]{0,63}\Z")
PAGE_QUERY_VALUE_PATTERN = re.compile(r"[A-Za-z0-9._~-]{1,128}\Z")


def parse_page_query(values: list[str] | tuple[str, ...] = ()) -> str:
    """Validate consumer-owned query parameters and encode one page suffix.

    The generic runner may select a bounded consumer mode, such as an
    explicitly opted-in renderer, without interpreting that mode. Keys and
    values stay ASCII and delimiter-free so the runner cannot be redirected
    to another path or origin through an argument.
    """
    if not isinstance(values, (list, tuple)):
        raise BrowserRuntimeError("page query parameters must be a sequence")
    if len(values) > MAX_PAGE_QUERY_PARAMETERS:
        raise BrowserRuntimeError(
            f"page query accepts at most {MAX_PAGE_QUERY_PARAMETERS} parameters"
        )
    pairs: list[tuple[str, str]] = []
    seen: set[str] = set()
    for raw in values:
        if not isinstance(raw, str):
            raise BrowserRuntimeError("page query parameter must be text")
        if len(raw.encode("utf-8")) > MAX_PAGE_QUERY_BYTES:
            raise BrowserRuntimeError("page query parameter exceeds the byte bound")
        key, separator, value = raw.partition("=")
        if not separator:
            raise BrowserRuntimeError("page query parameter must use KEY=VALUE")
        if (
            len(key.encode("ascii", errors="ignore")) > MAX_PAGE_QUERY_KEY_BYTES
            or not PAGE_QUERY_KEY_PATTERN.fullmatch(key)
        ):
            raise BrowserRuntimeError("page query key is outside the ASCII bound")
        if (
            len(value.encode("ascii", errors="ignore")) > MAX_PAGE_QUERY_VALUE_BYTES
            or not PAGE_QUERY_VALUE_PATTERN.fullmatch(value)
        ):
            raise BrowserRuntimeError("page query value is outside the ASCII bound")
        if key in seen:
            raise BrowserRuntimeError(f"page query key {key!r} is repeated")
        seen.add(key)
        pairs.append((key, value))
    if not pairs:
        return ""
    query = "?" + urllib.parse.urlencode(pairs)
    if len(query.encode("ascii")) > MAX_PAGE_QUERY_BYTES:
        raise BrowserRuntimeError("page query exceeds the byte bound")
    return query

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


def _cleanup_transfer(client: WebDriverClient) -> dict:
    result = client.execute(CLEANUP_TRANSFER)
    if result not in (
        {"listeners_removed": 4, "globals_cleared": True},
        {"listeners_removed": 0, "globals_cleared": True},
    ):
        raise BrowserRuntimeError(f"file transfer observer cleanup failed: {result!r}")
    return result


def run(args: argparse.Namespace) -> dict:
    engine, browser_name = resolve_browser_target(
        getattr(args, "engine", None),
        getattr(args, "browser_name", None),
    )
    if args.input == "chromium" and engine is not BrowserEngine.CHROMIUM:
        raise BrowserRuntimeError("the chromium input source requires the Chromium engine")
    lifecycle_cycles = validate_lifecycle_request(args)
    if getattr(args, "slice_controls", False) and lifecycle_cycles != 1:
        raise BrowserRuntimeError("slice controls require a single gallery lifecycle")
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
    keyboard_trace = (
        KeyboardTraceKind.parse(args.keyboard_trace)
        if getattr(args, "keyboard_trace", None) is not None
        else None
    )
    canvas_trace_path = None
    if args.canvas_trace is not None:
        canvas_trace_path = _safe_path(args.canvas_trace.resolve(), directory=ROOT / "output")
    if keyboard_trace and canvas_trace_path is None:
        raise BrowserRuntimeError("keyboard trace requires --canvas-trace")
    output = _safe_path(args.output.resolve(), directory=ROOT / "output")
    output.mkdir(parents=True, exist_ok=True)
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True, timeout=30).strip()
    client = WebDriverClient(args.driver_url, 120)
    canvas_trace: Trace | None = None
    lifecycle_canvas_traces: list[Trace] = []
    trace = Trace(engine, "", "disconnected", revision, {}, args.consumer_revision)
    document = {"status": "failed"}
    transfer_observer_installed = False
    try:
        with StaticServer(ROOT / "output" / "browser") as origin:
            trace.url = origin + "gallery.html" + parse_page_query(
                getattr(args, "page_query", ())
            )
            if getattr(args, "headless", False):
                client.create_session(browser_name, device_scale_milli, headless=True)
            else:
                client.create_session(browser_name, device_scale_milli)
            trace.metrics["headless"] = bool(getattr(args, "headless", False))
            trace.capabilities = {key: client.capabilities.get(key) for key in ("browserName", "browserVersion", "platformName")}
            client.set_timeouts(120_000)
            # The three range controls add a label and a native hit target below
            # the images; include that row in the gallery viewport capture.
            window_height = 1200 if getattr(args, "slice_controls", False) else 1100
            client._request("POST", client._session_path("window/rect"), {"width": 1440, "height": window_height})
            client.navigate(trace.url)
            _wait_for_text(client, "gallery-status", "Ready", timeout_ms=30_000, include=True)
            record_device_scale(client, trace, device_scale_milli)
            if getattr(args, "browser_memory_sample", False):
                browser_memory_sample(client, trace, "mounted")
            if lifecycle_cycles > 1:
                def new_canvas_trace() -> Trace:
                    cycle_trace = Trace(
                        engine,
                        trace.url,
                        "canvas",
                        revision,
                        client.capabilities,
                        args.consumer_revision,
                    )
                    record_device_scale(client, cycle_trace, device_scale_milli)
                    return cycle_trace

                def transfer_files(
                    cycle_client: WebDriverClient,
                    cycle_files: list[pathlib.Path],
                    point: dict,
                ) -> None:
                    if args.input == "chromium":
                        dispatch_files(cycle_client, cycle_files, point)
                    else:
                        select_files(cycle_client, cycle_files)

                run_gallery_lifecycle(
                    client,
                    trace,
                    new_canvas_trace,
                    files,
                    total,
                    expected_files,
                    oracle,
                    ids,
                    canvas_attributes,
                    args.input,
                    lifecycle_cycles,
                    canvas_trace_path.parent / "screenshots" / engine.value / "canvas",
                    transfer_files,
                    OBSERVE_TRANSFER,
                    _cleanup_transfer,
                    CANVAS_PIXELS,
                    lifecycle_canvas_traces,
                    args.lifecycle_timeout_seconds,
                    getattr(args, "browser_memory_sample", False),
                )
                trace.metrics["gallery_lifecycle_canvas_traces"] = [
                    (
                        canvas_trace_path.parent
                        / f"{canvas_trace_path.stem}-cycle-{cycle}{canvas_trace_path.suffix}"
                    ).relative_to(ROOT).as_posix()
                    for cycle in range(1, len(lifecycle_canvas_traces) + 1)
                ]
                viewport = client.execute(
                    "window.scrollTo(0,0); return "
                    "{width:innerWidth,height:innerHeight,device_scale:devicePixelRatio};"
                )
                document = trace.document()
                document["input_source"] = args.input
                document["files"] = len(files)
                document["bytes"] = total
                document["viewport"] = viewport
                document["file_manifest_sha256"] = hashlib.sha256(
                    json.dumps(expected_files, sort_keys=True).encode()
                ).hexdigest()
                document["oracle_sha256"] = hashlib.sha256(args.oracle.read_bytes()).hexdigest()
                document["assets"] = {
                    name: hashlib.sha256((ROOT / "output" / "browser" / name).read_bytes()).hexdigest()
                    for name in (
                        "gallery.html",
                        "gallery.js",
                        "gallery.css",
                        "consumer/ritk_snap.js",
                        "consumer/ritk_snap_bg.wasm",
                    )
                }
                document["sources"] = {
                    name: hashlib.sha256((ROOT / "scripts" / name).read_bytes()).hexdigest()
                    for name in (
                        "browser.py",
                        "browser_canvas.py",
                        "browser_drop.py",
                        "browser_drop_lifecycle.py",
                        "browser_file_read.py",
                        "browser_protocol.py",
                        "browser_runtime.py",
                        "browser_trace.py",
                    )
                }
                return document
            point = client.execute(OBSERVE_TRANSFER)
            transfer_observer_installed = True
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
            if getattr(args, "browser_memory_sample", False):
                browser_memory_sample(client, trace, "decoded")
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
                    keyboard_trace=keyboard_trace,
                    browser_memory=getattr(args, "browser_memory_sample", False),
                )
            if getattr(args, "slice_controls", False):
                from browser_gallery import capture_slice_gallery

                trace.metrics["slice_controls"] = capture_slice_gallery(
                    client,
                    output / "slices",
                    expected_counts={
                        canvas_id.removeprefix("ritk-snap-"): int(
                            oracle[canvas_id]["attributes"]["data-ritk-slice-count"]
                        )
                        for canvas_id in ids
                    },
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
            observer_cleanup = _cleanup_transfer(client)
            transfer_observer_installed = False
            trace.cleanup["transfer_observer"] = observer_cleanup
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
                for name in (
                    "browser.py",
                    "browser_canvas.py",
                    "browser_drop.py",
                    "browser_drop_lifecycle.py",
                    "browser_file_read.py",
                    "browser_gallery.py",
                    "browser_protocol.py",
                    "browser_runtime.py",
                    "browser_trace.py",
                )
            }
    except (BrowserRuntimeError, OSError, ValueError) as error:
        capture_error = None
        page_diagnostic = None
        if client.session_id is not None:
            capture_file_read_diagnostic(client, trace)
            try:
                screenshot(client, trace, output, "failure")
                page_diagnostic = client.execute(
                    "return {text:document.body.innerText.slice(0,8192),"
                    "events:window.metisFileEvidence};"
                )
            except BrowserRuntimeError as screenshot_error:
                capture_error = str(screenshot_error)
            if args.input == "chooser":
                capture_file_selection_diagnostic(client, trace, files)
        document = trace.document("failed")
        document["error"] = str(error)
        if page_diagnostic is not None:
            document["diagnostic"] = page_diagnostic
        if capture_error is not None:
            document["capture_error"] = capture_error
        raise
    finally:
        primary_error = sys.exc_info()[1]
        closed = False
        observer_cleanup_error = None
        if transfer_observer_installed and client.session_id is not None:
            try:
                _cleanup_transfer(client)
            except BrowserRuntimeError as cleanup_error:
                if primary_error is not None:
                    primary_error.add_note(
                        f"file transfer observer cleanup also failed: {cleanup_error}"
                    )
                else:
                    observer_cleanup_error = cleanup_error
                    document["status"] = "failed"
                    document.setdefault("cleanup", {})["transfer_observer_error"] = str(
                        cleanup_error
                    )
        try:
            client.close()
            closed = True
            document.setdefault("cleanup", {})["session_closed"] = True
        except BrowserRuntimeError as cleanup_error:
            document["status"] = "failed"
            document.setdefault("cleanup", {}).update(
                {"session_closed": False, "error": str(cleanup_error)}
            )
            if primary_error is None:
                raise
        finally:
            (output / "trace.json").write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
            if canvas_trace is not None and canvas_trace_path is not None:
                canvas_trace.cleanup["session_closed"] = closed
                _write_trace(canvas_trace_path, canvas_trace.document(document["status"]))
            if canvas_trace_path is not None:
                for cycle, cycle_trace in enumerate(lifecycle_canvas_traces, start=1):
                    cycle_trace.cleanup["session_closed"] = closed
                    cycle_path = canvas_trace_path.parent / (
                        f"{canvas_trace_path.stem}-cycle-{cycle}{canvas_trace_path.suffix}"
                    )
                    _write_trace(cycle_path, cycle_trace.document(document["status"]))
        if observer_cleanup_error is not None:
            raise observer_cleanup_error
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
    parser.add_argument(
        "--page-query",
        action="append",
        default=[],
        metavar="KEY=VALUE",
        help="bounded consumer query parameter; may be repeated up to eight times",
    )
    parser.add_argument("--canvas-trace", type=pathlib.Path,
                        help="write a paired trusted canvas trace after the file drop")
    parser.add_argument(
        "--keyboard-trace",
        nargs="?",
        const="navigation",
        choices=("navigation", "cine-rate"),
        metavar="{navigation,cine-rate}",
        help="include focused keyboard evidence; default profile is ArrowDown navigation",
    )
    parser.add_argument("--canvas-attribute", action="append", default=[],
                        help="consumer-selected data-* attribute for the paired canvas trace")
    parser.add_argument("--browser-memory-sample", action="store_true",
                        help="record bounded measureUserAgentSpecificMemory observations when exposed")
    parser.add_argument("--slice-controls", action="store_true",
                        help="verify gallery range controls through trusted keyboard and pointer input")
    parser.add_argument("--headless", action="store_true",
                        help="isolate browser automation from desktop mouse and keyboard input")
    parser.add_argument("--input", choices=("manual", "chooser", "chromium"), default="manual",
                        help="manual OS drop, standard W3C chooser, or Chromium CDP drag")
    parser.add_argument(
        "--lifecycle-cycles",
        type=int,
        default=1,
        metavar="1..8",
        help="bounded same-page gallery lifecycle cycles; repeated mode requires cine trace",
    )
    parser.add_argument(
        "--lifecycle-timeout-seconds",
        type=int,
        default=300,
        metavar="1..300",
        help="monotonic bound for the complete repeated gallery lifecycle suite",
    )
    parser.add_argument("--output", type=pathlib.Path, default=ROOT / "output" / "browser" / "drop")
    run(parser.parse_args())


if __name__ == "__main__":
    main()
