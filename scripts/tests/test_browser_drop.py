"""File-backed browser proof input and gallery delivery contracts."""
from __future__ import annotations

import json
import pathlib
import shutil
import subprocess
import sys
import tempfile
import types
import unittest
from unittest import mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from browser_drop import (
    CLEANUP_TRANSFER,
    MAX_FILE_BYTES,
    MAX_BATCH_BYTES,
    MAX_FILES,
    OBSERVE_TRANSFER,
    build_parser,
    _capture_consumer_after_host_probes,
    parse_page_query,
    resolve_browser_target,
    run,
    study_files,
)
from browser_canvas_capture import (
    CanvasCaptureMode,
    compare_screenshot_stability,
    validate_context_name,
)
from browser_drop_lifecycle import (
    MAX_LIFECYCLE_CYCLES,
    _DeadlineClient,
    _stopped_cleanup,
    assert_lifecycle_growth,
    validate_lifecycle_request,
)
from browser_file_read import (
    CLEANUP_SELECTION_DIAGNOSTIC,
    INSTALL_SELECTION_DIAGNOSTIC,
    MAX_READ_DIAGNOSTIC_BYTES,
    OBSERVE_SELECTION_DIAGNOSTIC,
    READ_DIAGNOSTIC_TIMEOUT_MS,
    READ_FAILURE_DIAGNOSTIC,
    RESET_SELECTION_DIAGNOSTIC,
    capture_file_read_diagnostic,
    capture_file_selection_diagnostic,
)
from browser_protocol import BrowserRuntimeError
from browser_trace import BrowserEngine, Trace


NODE_READ_DIAGNOSTIC_HARNESS = r"""
const mode = process.argv[2];
const diagnostic = process.argv[1];
let releasedLocks = 0;
let revokedUrls = 0;
const fileReaders = [];
class SelectedFile {
  constructor() { this.size = 4; }
  arrayBuffer() { return Promise.resolve(new ArrayBuffer(this.size)); }
  slice(_start, end) {
    return {
      size: end,
      arrayBuffer: () => Promise.resolve(new ArrayBuffer(end))
    };
  }
}
class DiagnosticFileReader {
  static EMPTY = 0;
  static LOADING = 1;
  static DONE = 2;
  constructor() {
    this.readyState = DiagnosticFileReader.EMPTY;
    this.listeners = new Map();
    this.result = null;
    this.error = null;
    fileReaders.push(this);
  }
  addEventListener(name, listener) { this.listeners.set(name, listener); }
  removeEventListener(name, listener) {
    if (this.listeners.get(name) === listener) this.listeners.delete(name);
  }
  readAsArrayBuffer(blob) {
    if (mode === 'synchronous-reader-error') {
      throw new DOMException('', 'SecurityError');
    }
    this.readyState = DiagnosticFileReader.LOADING;
    this.result = new ArrayBuffer(blob.size);
    queueMicrotask(() => {
      this.readyState = DiagnosticFileReader.DONE;
      const loaded = this.listeners.get('load');
      if (loaded) loaded();
    });
  }
  abort() {
    this.readyState = DiagnosticFileReader.DONE;
    const aborted = this.listeners.get('abort');
    if (aborted) aborted();
  }
}
const streamReader = {
  read: () => mode === 'cleanup-race'
    ? new Promise(resolve => setTimeout(() => resolve({done: true}), 4900))
    : Promise.resolve({done: true}),
  cancel: () => {
    if (mode === 'delayed-cancel-error') {
      return new Promise((_resolve, reject) => setImmediate(
        () => reject(new DOMException('', 'AbortError'))
      ));
    }
    if (mode === 'cleanup-race') {
      return new Promise((_resolve, reject) => setTimeout(
        () => reject(new DOMException('', 'AbortError')), 300
      ));
    }
    return Promise.resolve();
  },
  releaseLock: () => { releasedLocks += 1; }
};
global.File = SelectedFile;
global.FileReader = DiagnosticFileReader;
global.window = {metisSelectedFile: new SelectedFile()};
Object.defineProperty(globalThis, 'crypto', {value: {
  subtle: {digest: () => Promise.resolve(new ArrayBuffer(32))}
}});
global.URL = {
  createObjectURL: () => 'blob:diagnostic',
  revokeObjectURL: () => { revokedUrls += 1; }
};
global.fetch = () => Promise.resolve({
  ok: true,
  body: {getReader: () => streamReader}
});
const execute = new Function(diagnostic);
execute(result => {
  result.harness = {
    retained_listeners: fileReaders.reduce((count, reader) => count + reader.listeners.size, 0),
    released_locks: releasedLocks,
    revoked_urls: revokedUrls
  };
  process.stdout.write(JSON.stringify(result));
});
"""

NODE_SELECTION_CLEANUP_HARNESS = r"""
const install = new Function(process.argv[1]);
const cleanup = new Function(process.argv[2]);
const prior = {kind: 'prior-selected-file'};
let input;
let label;
global.window = {metisSelectedFile: prior};
global.document = {
  getElementById: () => null,
  createElement: name => {
    if (name === 'input') {
      input = {
        isConnected: false,
        addEventListener: () => {},
        files: {length: 0, item: () => null}
      };
      return input;
    }
    label = {
      isConnected: false,
      append: () => {},
      remove: () => {
        label.isConnected = false;
        input.isConnected = false;
      }
    };
    return label;
  },
  body: {
    append: () => {
      label.isConnected = true;
      input.isConnected = true;
      throw new DOMException('', 'HierarchyRequestError');
    }
  }
};
let installException = null;
try {
  install();
} catch (error) {
  installException = error.name;
}
const cleanupResult = cleanup();
process.stdout.write(JSON.stringify({
  install_exception: installException,
  cleanup: cleanupResult,
  prior_restored: window.metisSelectedFile === prior,
  control_removed: !input.isConnected && !label.isConnected,
  state_removed: !Object.prototype.hasOwnProperty.call(
    window, 'metisFileSelectionDiagnostic'
  )
}));
"""

NODE_TRANSFER_CLEANUP_HARNESS = r"""
const observe = new Function(process.argv[1]);
const cleanup = new Function(process.argv[2]);
const listeners = new Map();
const makeTarget = () => ({
  files: [],
  addEventListener(type, listener) {
    const registrations = listeners.get(type) || [];
    registrations.push(listener);
    listeners.set(type, registrations);
  },
  removeEventListener(type, listener) {
    listeners.set(type, (listeners.get(type) || []).filter(item => item !== listener));
  },
  getBoundingClientRect() {
    return {x: 0, y: 0, width: 100, height: 100, bottom: 100};
  }
});
const zone = makeTarget();
const input = makeTarget();
global.window = {};
global.innerHeight = 200;
global.document = {getElementById: id => id === 'drop-zone' ? zone : input};
observe();
observe();
const result = cleanup();
process.stdout.write(JSON.stringify({
  result,
  retained: Array.from(listeners.values()).reduce((count, items) => count + items.length, 0),
  evidence: Object.prototype.hasOwnProperty.call(window, 'metisFileEvidence'),
  files: Object.prototype.hasOwnProperty.call(window, 'metisInputFiles'),
  selected: Object.prototype.hasOwnProperty.call(window, 'metisSelectedFile'),
  guard: Object.prototype.hasOwnProperty.call(window, 'metisTransferObserver')
}));
"""


class FileDropTests(unittest.TestCase):
    @staticmethod
    def _lifecycle_records(capacities=(65_536, 131_072, 131_072, 131_072)):
        records = []
        for cycle, capacity in enumerate(capacities, start=1):
            phases = []
            for phase in ("mounted", "transfer", "decoded", "cine"):
                phases.append({
                    "phase": phase,
                    "gallery": {
                        "wasm_bytes": capacity,
                        "host_listeners": 7,
                        "consumer_listeners": 5,
                        "mounted": True,
                    },
                })
            phases.append({
                "phase": "stopped",
                "gallery": {
                    "wasm_bytes": capacity,
                    "host_listeners": 0,
                    "consumer_listeners": 0,
                    "mounted": False,
                },
            })
            records.append({"cycle": cycle, "status": "complete", "phases": phases})
        return records

    def test_browser_target_resolution_covers_the_matrix(self):
        self.assertEqual(resolve_browser_target(None, "chrome"), (BrowserEngine.CHROMIUM, "chrome"))
        self.assertEqual(resolve_browser_target("chromium", "MicrosoftEdge"), (BrowserEngine.CHROMIUM, "MicrosoftEdge"))
        self.assertEqual(resolve_browser_target("firefox", None), (BrowserEngine.FIREFOX, "firefox"))
        self.assertEqual(resolve_browser_target("webkit", "safari"), (BrowserEngine.WEBKIT, "safari"))
        with self.assertRaisesRegex(BrowserRuntimeError, "incompatible"):
            resolve_browser_target("firefox", "chrome")

    def test_consumer_capture_is_single_lifecycle_only(self):
        args = types.SimpleNamespace(
            engine="chromium",
            browser_name=None,
            input="chooser",
            lifecycle_cycles=4,
            canvas_trace=pathlib.Path("output/browser/trace.json"),
            keyboard_trace="cine-rate",
            canvas_capture="rgba",
        )
        with self.assertRaisesRegex(BrowserRuntimeError, "single gallery lifecycle"):
            run(args, consumer_capture=lambda *_: {})

    def test_host_rejections_run_before_consumer_teardown(self):
        events = []

        class Client:
            def find(self, selector):
                events.append(("find", selector))
                return selector

            def execute_async(self, _script, _arguments):
                events.append("frame")
                return {"rgba_sha256": "stable"}

        def rejection_probe(client, _trace, _point, _input_source):
            events.append("rejections")
            client.find("#file-input")

        def consumer_capture(_client, _output, _oracle, _canvas_ids):
            events.append("consumer")
            return {"teardown": True}

        trace = types.SimpleNamespace(metrics={})
        with mock.patch("browser_drop.check_rejections", side_effect=rejection_probe):
            _capture_consumer_after_host_probes(
                Client(),
                trace,
                {},
                "chooser",
                CanvasCaptureMode.RGBA,
                None,
                ("viewer",),
                {"viewer": {"rgba_sha256": "stable"}},
                {},
                None,
                consumer_capture,
                pathlib.Path("output/browser"),
            )

        self.assertEqual(events, ["rejections", ("find", "#file-input"), "frame", "consumer"])
        self.assertEqual(trace.metrics, {"consumer_capture": {"teardown": True}})

    def test_generic_parser_has_no_consumer_specific_slice_option(self):
        parser = build_parser()
        self.assertNotIn("--slice-controls", parser._option_string_actions)

    def test_page_query_is_encoded_as_one_bounded_consumer_suffix(self):
        self.assertEqual(
            parse_page_query(["renderer=webgpu", "profile=clinical-view"]),
            "?renderer=webgpu&profile=clinical-view",
        )
        self.assertEqual(parse_page_query(()), "")

    def test_page_query_rejects_ambiguous_or_unsafe_parameters(self):
        invalid = (
            ("missing separator", "renderer"),
            ("duplicate", "mode=raster", "mode=webgpu"),
            ("empty value", "mode="),
            ("unsafe key", "mode/name=raster"),
            ("unsafe value", "mode=raster&debug=true"),
            ("non-text", 7),
        )
        for label, *values in invalid:
            with self.subTest(label=label), self.assertRaisesRegex(
                BrowserRuntimeError, "page query"
            ):
                parse_page_query(values)
        with self.assertRaisesRegex(BrowserRuntimeError, "at most 8"):
            parse_page_query([f"mode{index}=value" for index in range(9)])

    def test_canvas_capture_modes_are_explicit(self):
        self.assertIs(CanvasCaptureMode.parse("rgba"), CanvasCaptureMode.RGBA)
        self.assertIs(CanvasCaptureMode.parse("screenshot"), CanvasCaptureMode.SCREENSHOT)
        with self.assertRaisesRegex(BrowserRuntimeError, "canvas capture mode"):
            CanvasCaptureMode.parse("webgpu")
        self.assertEqual(validate_context_name("webgpu"), "webgpu")
        with self.assertRaisesRegex(BrowserRuntimeError, "canvas context"):
            validate_context_name("WebGPU")

    def test_screenshot_stability_reuses_the_canvas_element(self):
        class Client:
            def __init__(self):
                self.selectors = []

            def execute(self, _script, arguments):
                return {"width": 512, "height": 512, "context": arguments[1]}

            def find(self, selector):
                self.selectors.append(selector)
                return selector

        client = Client()
        trace = Trace(BrowserEngine.CHROMIUM, "", "", "0" * 40, {})
        observations = {
            "viewer-axial": {
                "width": 512,
                "height": 512,
                "screenshot_sha256": "stable",
            }
        }

        def record_screenshot(_client, target, _directory, label, _element):
            target.screenshots.append({
                "label": label,
                "sha256": "stable",
                "width": 1024,
                "height": 1024,
                "bytes": 128,
            })

        with mock.patch(
            "browser_canvas_capture._element_screenshot",
            side_effect=record_screenshot,
        ):
            compare_screenshot_stability(
                client,
                trace,
                pathlib.Path("output/browser/test"),
                ("viewer-axial",),
                observations,
                "webgpu",
            )

        self.assertEqual(client.selectors, ["#viewer-axial"])
        self.assertEqual(trace.screenshots[0]["label"], "viewer-axial-after-rejections")

    def test_file_selection_preserves_exact_bytes_and_ignores_other_names(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "b.bin").write_bytes(b"second")
            (root / "a.bin").write_bytes(b"first")
            (root / "LICENSE").write_text("license", encoding="utf-8")
            files, total = study_files(root, "*.bin")
            self.assertEqual([path.name for path in files], ["a.bin", "b.bin"])
            self.assertEqual(total, 11)

    def test_empty_folder_nested_folder_and_traversal_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            with self.assertRaisesRegex(BrowserRuntimeError, "empty"):
                study_files(root)
            (root / "nested").mkdir()
            with self.assertRaisesRegex(BrowserRuntimeError, "files only"):
                study_files(root)
            with self.assertRaisesRegex(BrowserRuntimeError, "immediate"):
                study_files(root, "../*")

    def test_file_and_batch_boundaries(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            for index in range(MAX_BATCH_BYTES // MAX_FILE_BYTES):
                with (root / str(index)).open("wb") as file:
                    file.truncate(MAX_FILE_BYTES)
            self.assertEqual(study_files(root)[1], MAX_BATCH_BYTES)
            (root / "extra").write_bytes(b"x")
            with self.assertRaisesRegex(BrowserRuntimeError, "bounds"):
                study_files(root)
            with (root / "large").open("wb") as file:
                file.truncate(MAX_FILE_BYTES + 1)
            with self.assertRaisesRegex(BrowserRuntimeError, "bounds"):
                study_files(root, "large")

    def test_file_count_boundary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            for index in range(MAX_FILES):
                (root / str(index)).write_bytes(b"x")
            files, total = study_files(root)
            self.assertEqual((len(files), total), (MAX_FILES, MAX_FILES))
            (root / "extra").write_bytes(b"x")
            with self.assertRaisesRegex(BrowserRuntimeError, "bounds"):
                study_files(root)

    def test_transfer_observer_covers_drop_and_standard_chooser(self):
        self.assertIn("input.addEventListener('change'", OBSERVE_TRANSFER)
        self.assertIn("event.dataTransfer", OBSERVE_TRANSFER)
        self.assertIn("window.metisInputFiles", OBSERVE_TRANSFER)
        self.assertIn("window.metisSelectedFile = list.item ? list.item(0) : list[0]", OBSERVE_TRANSFER)
        self.assertIn("const evidence = (async () =>", OBSERVE_TRANSFER)
        self.assertIn("error.name", OBSERVE_TRANSFER)
        self.assertNotIn("dispatchEvent", OBSERVE_TRANSFER)

    @unittest.skipUnless(shutil.which("node"), "Node.js is required for browser-script cleanup")
    def test_transfer_observer_reinstall_and_teardown_release_every_reference(self):
        process = subprocess.run(
            ["node", "-e", NODE_TRANSFER_CLEANUP_HARNESS, OBSERVE_TRANSFER, CLEANUP_TRANSFER],
            check=True,
            capture_output=True,
            text=True,
            timeout=10,
        )
        result = json.loads(process.stdout)
        self.assertEqual(
            result,
            {
                "result": {"listeners_removed": 4, "globals_cleared": True},
                "retained": 0,
                "evidence": False,
                "files": False,
                "selected": False,
                "guard": False,
            },
        )

    def test_repeated_lifecycle_request_is_bounded_and_requires_cine(self):
        valid = types.SimpleNamespace(
            lifecycle_cycles=4,
            lifecycle_timeout_seconds=300,
            input="chooser",
            canvas_trace=pathlib.Path("canvas.json"),
            keyboard_trace="cine-rate",
        )
        self.assertEqual(validate_lifecycle_request(valid), 4)
        for value in (0, MAX_LIFECYCLE_CYCLES + 1, True):
            valid.lifecycle_cycles = value
            with self.subTest(value=value), self.assertRaisesRegex(
                BrowserRuntimeError, "lifecycle-cycles must be between"
            ):
                validate_lifecycle_request(valid)
        valid.lifecycle_cycles = 3
        with self.assertRaisesRegex(BrowserRuntimeError, "at least 4 cycles"):
            validate_lifecycle_request(valid)
        valid.lifecycle_cycles = 4
        valid.keyboard_trace = "navigation"
        with self.assertRaisesRegex(BrowserRuntimeError, "cine-rate"):
            validate_lifecycle_request(valid)
        valid.keyboard_trace = "cine-rate"
        valid.lifecycle_timeout_seconds = 301
        with self.assertRaisesRegex(BrowserRuntimeError, "between 1 and 300"):
            validate_lifecycle_request(valid)

    def test_growth_gate_accepts_stable_guards_and_post_warmup_capacity(self):
        assert_lifecycle_growth(self._lifecycle_records())

    def test_growth_gate_rejects_wasm_growth_after_two_warmups(self):
        with self.assertRaisesRegex(BrowserRuntimeError, "capacity grew after warmup"):
            assert_lifecycle_growth(
                self._lifecycle_records((65_536, 131_072, 131_072, 196_608))
            )

    def test_growth_gate_rejects_retained_stopped_listener_guard(self):
        records = self._lifecycle_records()
        records[-1]["phases"][-1]["gallery"]["consumer_listeners"] = 1
        with self.assertRaisesRegex(BrowserRuntimeError, "retained listener guards"):
            assert_lifecycle_growth(records)

    def test_growth_gate_rejects_empty_missing_and_reordered_phases(self):
        empty = self._lifecycle_records()
        empty[0]["phases"] = []
        with self.assertRaisesRegex(BrowserRuntimeError, "omitted or reordered"):
            assert_lifecycle_growth(empty)
        missing = self._lifecycle_records()
        missing[0]["phases"].pop(1)
        with self.assertRaisesRegex(BrowserRuntimeError, "omitted or reordered"):
            assert_lifecycle_growth(missing)
        reordered = self._lifecycle_records()
        reordered[0]["phases"][1], reordered[0]["phases"][2] = (
            reordered[0]["phases"][2],
            reordered[0]["phases"][1],
        )
        with self.assertRaisesRegex(BrowserRuntimeError, "omitted or reordered"):
            assert_lifecycle_growth(reordered)

    def test_growth_gate_rejects_incomplete_cycle_and_invalid_wasm_page_capacity(self):
        incomplete = self._lifecycle_records()
        incomplete[-1]["status"] = "running"
        with self.assertRaisesRegex(BrowserRuntimeError, "sequential and complete"):
            assert_lifecycle_growth(incomplete)
        invalid_capacity = self._lifecycle_records()
        invalid_capacity[-1]["phases"][0]["gallery"]["wasm_bytes"] = 1
        with self.assertRaisesRegex(BrowserRuntimeError, "invalid wasm_bytes"):
            assert_lifecycle_growth(invalid_capacity)

    def test_stopped_cleanup_requires_removed_controls_and_globals(self):
        expected = {
            "transfer_observer_cleared": True,
            "file_evidence_cleared": True,
            "input_files_cleared": True,
            "selected_file_cleared": True,
            "canvas_trace_cleared": True,
            "file_input_removed": True,
            "drop_zone_removed": True,
        }
        client = types.SimpleNamespace(execute=lambda _script: expected)
        self.assertEqual(_stopped_cleanup(client), expected)
        client = types.SimpleNamespace(execute=lambda _script: {**expected, "drop_zone_removed": False})
        with self.assertRaisesRegex(BrowserRuntimeError, "retained browser resources"):
            _stopped_cleanup(client)

    def test_suite_deadline_rejects_commands_after_expiry(self):
        client = types.SimpleNamespace(_timeout=120.0)
        bounded = _DeadlineClient(client, 0.0)
        with self.assertRaisesRegex(BrowserRuntimeError, "suite deadline exceeded"):
            bounded.remaining_milliseconds()
        bounded.restore()
        self.assertEqual(client._timeout, 120.0)

    def test_failure_diagnostic_compares_four_bounded_real_file_reads(self):
        self.assertEqual(MAX_READ_DIAGNOSTIC_BYTES, 1_048_576)
        self.assertEqual(READ_DIAGNOSTIC_TIMEOUT_MS, 5_000)
        self.assertIn(f"const maxBytes = {MAX_READ_DIAGNOSTIC_BYTES}", READ_FAILURE_DIAGNOSTIC)
        self.assertIn(f"const timeoutMs = {READ_DIAGNOSTIC_TIMEOUT_MS}", READ_FAILURE_DIAGNOSTIC)
        self.assertEqual(READ_FAILURE_DIAGNOSTIC.count("await probe("), 4)
        self.assertIn("return file.arrayBuffer()", READ_FAILURE_DIAGNOSTIC)
        self.assertIn("file.slice(0, Math.min(file.size, maxBytes))", READ_FAILURE_DIAGNOSTIC)
        self.assertIn("bounded.arrayBuffer()", READ_FAILURE_DIAGNOSTIC)
        self.assertIn("reader.readAsArrayBuffer(bounded)", READ_FAILURE_DIAGNOSTIC)
        self.assertIn("fetch(objectUrl, {signal: controller.signal})", READ_FAILURE_DIAGNOSTIC)
        self.assertIn("response.body.getReader()", READ_FAILURE_DIAGNOSTIC)
        for probe_name in (
            "original_file_array_buffer",
            "bounded_slice_array_buffer",
            "bounded_slice_file_reader",
            "bounded_blob_url_stream",
        ):
            self.assertIn(probe_name, READ_FAILURE_DIAGNOSTIC)
        self.assertNotIn("new File(", READ_FAILURE_DIAGNOSTIC)
        self.assertNotIn("file.name", READ_FAILURE_DIAGNOSTIC)
        self.assertNotIn("error.message", READ_FAILURE_DIAGNOSTIC)

    def test_failure_diagnostic_times_out_and_releases_browser_resources(self):
        self.assertIn("setTimeout(() =>", READ_FAILURE_DIAGNOSTIC)
        self.assertIn("reader.abort()", READ_FAILURE_DIAGNOSTIC)
        for event in ("load", "error", "abort"):
            self.assertIn(f"reader.removeEventListener('{event}'", READ_FAILURE_DIAGNOSTIC)
        self.assertIn("readerToRelease.cancel()", READ_FAILURE_DIAGNOSTIC)
        self.assertIn("readerToRelease.releaseLock()", READ_FAILURE_DIAGNOSTIC)
        self.assertIn("if (fetchCleanupPromise) return fetchCleanupPromise", READ_FAILURE_DIAGNOSTIC)
        self.assertIn("controllerToAbort.abort()", READ_FAILURE_DIAGNOSTIC)
        self.assertIn("URL.revokeObjectURL(urlToRevoke)", READ_FAILURE_DIAGNOSTIC)
        self.assertIn("cleanup_exceptions: cleanupExceptions", READ_FAILURE_DIAGNOSTIC)
        self.assertNotIn("catch(() => {})", READ_FAILURE_DIAGNOSTIC)

    def test_failure_diagnostic_is_appended_to_the_trace(self):
        expected = {
            "status": "captured",
            "probes": {"original_file_array_buffer": {"bytes": 4, "sha256": "0" * 64}},
        }

        class Client:
            @staticmethod
            def execute_async(script, arguments=()):
                self.assertEqual(script, READ_FAILURE_DIAGNOSTIC)
                self.assertEqual(arguments, ())
                return expected

        trace = types.SimpleNamespace(actions=[])
        capture_file_read_diagnostic(Client(), trace)
        self.assertEqual(trace.actions, [{"file_read_diagnostic": expected}])

    def test_failure_diagnostic_protocol_error_is_redacted(self):
        class Client:
            @staticmethod
            def execute_async(_script, _arguments=()):
                raise BrowserRuntimeError("C:\\private\\study\\manifest.bin")

        trace = types.SimpleNamespace(actions=[])
        capture_file_read_diagnostic(Client(), trace)
        self.assertEqual(
            trace.actions,
            [{"file_read_diagnostic": {"status": "unavailable", "reason": "webdriver-error"}}],
        )

    def test_isolated_selection_diagnostic_uses_two_real_path_selections(self):
        class Client:
            def __init__(self):
                self.calls = []
                self.sent = []
                self.selected = ()
                self.changes = 0

            def execute(self, script):
                self.calls.append(("execute", script))
                if script == INSTALL_SELECTION_DIAGNOSTIC:
                    return {"id": "metis-file-selection-diagnostic-7"}
                if script == OBSERVE_SELECTION_DIAGNOSTIC:
                    return {"files": len(self.selected), "changes": self.changes}
                if script == RESET_SELECTION_DIAGNOSTIC:
                    self.selected = ()
                    return {"files": 0, "changes": self.changes}
                if script == CLEANUP_SELECTION_DIAGNOSTIC:
                    return {"removed": True, "restored": True}
                raise AssertionError("unexpected browser script")

            def find(self, selector):
                self.calls.append(("find", selector))
                return "diagnostic-element"

            def send_file_paths(self, element, paths):
                self.calls.append(("send", element))
                self.sent.append(paths)
                self.selected = paths
                self.changes += 1

            def execute_async(self, script, arguments=()):
                self.calls.append(("execute_async", script, arguments))
                return {"status": "captured", "selected_bytes": len(self.selected)}

        files = [pathlib.Path("first.dcm"), pathlib.Path("second.dcm")]
        client = Client()
        trace = types.SimpleNamespace(actions=[])
        capture_file_selection_diagnostic(client, trace, files)
        self.assertEqual(client.sent[0], [files[0]])
        self.assertIs(client.sent[0][0], files[0])
        self.assertIs(client.sent[1], files)
        self.assertEqual(
            [call[0] for call in client.calls],
            [
                "execute",
                "find",
                "send",
                "execute",
                "execute_async",
                "execute",
                "send",
                "execute",
                "execute_async",
                "execute",
            ],
        )
        result = trace.actions[0]["file_selection_diagnostic"]
        self.assertEqual([selection["requested_files"] for selection in result["selections"]], [1, 2])
        self.assertEqual([selection["observation"]["files"] for selection in result["selections"]], [1, 2])
        self.assertEqual(result["cleanup"], {"removed": True, "restored": True})
        serialized = json.dumps(result)
        self.assertNotIn("first.dcm", serialized)
        self.assertNotIn("second.dcm", serialized)

    def test_isolated_selection_diagnostic_cleans_up_after_selection_error(self):
        class Client:
            def __init__(self):
                self.send_count = 0
                self.cleanup_calls = 0

            def execute(self, script):
                if script == INSTALL_SELECTION_DIAGNOSTIC:
                    return {"id": "metis-file-selection-diagnostic-9"}
                if script == OBSERVE_SELECTION_DIAGNOSTIC:
                    return {"files": 1, "changes": 1}
                if script == RESET_SELECTION_DIAGNOSTIC:
                    return {"files": 0, "changes": 1}
                if script == CLEANUP_SELECTION_DIAGNOSTIC:
                    self.cleanup_calls += 1
                    return {"removed": True, "restored": True}
                raise AssertionError("unexpected browser script")

            @staticmethod
            def find(_selector):
                return "diagnostic-element"

            def send_file_paths(self, _element, _paths):
                self.send_count += 1
                if self.send_count == 2:
                    raise BrowserRuntimeError("C:\\private\\study\\second.dcm")

            @staticmethod
            def execute_async(_script, _arguments=()):
                return {"status": "captured", "selected_bytes": 4}

        client = Client()
        trace = types.SimpleNamespace(actions=[])
        capture_file_selection_diagnostic(
            client, trace, [pathlib.Path("first.dcm"), pathlib.Path("second.dcm")]
        )
        self.assertEqual(client.cleanup_calls, 1)
        result = trace.actions[0]["file_selection_diagnostic"]
        self.assertEqual(result["status"], "unavailable")
        self.assertEqual(result["reason"], "webdriver-error")
        self.assertEqual(len(result["selections"]), 1)
        self.assertNotIn("private", json.dumps(result))

    @unittest.skipUnless(shutil.which("node"), "Node.js is required for browser-script fault injection")
    def test_isolated_control_append_failure_restores_prior_handle(self):
        process = subprocess.run(
            [
                "node",
                "-e",
                NODE_SELECTION_CLEANUP_HARNESS,
                INSTALL_SELECTION_DIAGNOSTIC,
                CLEANUP_SELECTION_DIAGNOSTIC,
            ],
            check=True,
            capture_output=True,
            text=True,
            timeout=10,
        )
        result = json.loads(process.stdout)
        self.assertEqual(result["install_exception"], "HierarchyRequestError")
        self.assertEqual(result["cleanup"], {"removed": True, "restored": True})
        self.assertTrue(result["prior_restored"])
        self.assertTrue(result["control_removed"])
        self.assertTrue(result["state_removed"])

    def test_isolated_selection_control_restores_prior_handle_and_uses_no_replacement_files(self):
        scripts = "\n".join(
            (
                INSTALL_SELECTION_DIAGNOSTIC,
                OBSERVE_SELECTION_DIAGNOSTIC,
                RESET_SELECTION_DIAGNOSTIC,
                CLEANUP_SELECTION_DIAGNOSTIC,
            )
        )
        self.assertIn("document.createElement('input')", INSTALL_SELECTION_DIAGNOSTIC)
        self.assertIn("document.createElement('label')", INSTALL_SELECTION_DIAGNOSTIC)
        self.assertIn("label.append(input)", INSTALL_SELECTION_DIAGNOSTIC)
        self.assertIn("document.body.append(label)", INSTALL_SELECTION_DIAGNOSTIC)
        self.assertIn("input.type = 'file'", INSTALL_SELECTION_DIAGNOSTIC)
        self.assertIn("input.multiple = true", INSTALL_SELECTION_DIAGNOSTIC)
        self.assertNotIn("input.hidden", INSTALL_SELECTION_DIAGNOSTIC)
        self.assertIn("priorSelectedFile: window.metisSelectedFile", INSTALL_SELECTION_DIAGNOSTIC)
        self.assertLess(
            INSTALL_SELECTION_DIAGNOSTIC.index("window.metisFileSelectionDiagnostic = state"),
            INSTALL_SELECTION_DIAGNOSTIC.index("window.metisSelectedFile = null"),
        )
        self.assertIn("window.metisSelectedFile = null", INSTALL_SELECTION_DIAGNOSTIC)
        self.assertIn("window.metisSelectedFile = state.priorSelectedFile", CLEANUP_SELECTION_DIAGNOSTIC)
        self.assertIn("state.label.remove()", CLEANUP_SELECTION_DIAGNOSTIC)
        self.assertNotIn("new File(", scripts)
        self.assertNotIn("new Blob(", scripts)

    @staticmethod
    def _execute_diagnostic_fault(mode):
        # File-backed source keeps Windows process argument transport bounded.
        file_harness = NODE_READ_DIAGNOSTIC_HARNESS.replace(
            "const mode = process.argv[2];\nconst diagnostic = process.argv[1];",
            "const mode = process.argv[3];\n"
            "const diagnostic = require('node:fs').readFileSync(process.argv[2], 'utf8');",
            1,
        )
        with tempfile.TemporaryDirectory(prefix="metis-node-diagnostic-") as directory:
            root = pathlib.Path(directory)
            harness_path = root / "harness.js"
            diagnostic_path = root / "diagnostic.js"
            harness_path.write_text(file_harness, encoding="utf-8")
            diagnostic_path.write_text(READ_FAILURE_DIAGNOSTIC, encoding="utf-8")
            process = subprocess.run(
                ["node", str(harness_path), str(diagnostic_path), mode],
                check=True,
                capture_output=True,
                text=True,
                timeout=10,
            )
        return json.loads(process.stdout)

    @unittest.skipUnless(shutil.which("node"), "Node.js is required for browser-script fault injection")
    def test_synchronous_file_reader_failure_removes_every_listener(self):
        result = self._execute_diagnostic_fault("synchronous-reader-error")
        self.assertEqual(
            result["probes"]["bounded_slice_file_reader"], {"exception": "SecurityError"}
        )
        self.assertEqual(result["harness"]["retained_listeners"], 0)
        self.assertEqual(result["harness"]["released_locks"], 1)
        self.assertEqual(result["harness"]["revoked_urls"], 1)

    @unittest.skipUnless(shutil.which("node"), "Node.js is required for browser-script fault injection")
    def test_delayed_stream_cancel_failure_precedes_diagnostic_callback(self):
        result = self._execute_diagnostic_fault("delayed-cancel-error")
        self.assertEqual(
            result["cleanup_exceptions"],
            [{"operation": "stream-cancel", "exception": "AbortError"}],
        )
        self.assertEqual(result["harness"]["retained_listeners"], 0)
        self.assertEqual(result["harness"]["released_locks"], 1)
        self.assertEqual(result["harness"]["revoked_urls"], 1)

    @unittest.skipUnless(shutil.which("node"), "Node.js is required for browser-script fault injection")
    def test_probe_timeout_awaits_in_flight_stream_cleanup(self):
        result = self._execute_diagnostic_fault("cleanup-race")
        self.assertEqual(
            result["probes"]["bounded_blob_url_stream"], {"exception": "TimeoutError"}
        )
        self.assertEqual(
            result["cleanup_exceptions"],
            [{"operation": "stream-cancel", "exception": "AbortError"}],
        )
        self.assertEqual(result["harness"]["released_locks"], 1)
        self.assertEqual(result["harness"]["revoked_urls"], 1)


if __name__ == "__main__":
    unittest.main()
