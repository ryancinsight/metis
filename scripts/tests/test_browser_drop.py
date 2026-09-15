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

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from browser_drop import (
    MAX_FILE_BYTES,
    MAX_BATCH_BYTES,
    MAX_FILES,
    OBSERVE_TRANSFER,
    resolve_browser_target,
    study_files,
)
from browser_file_read import (
    MAX_READ_DIAGNOSTIC_BYTES,
    READ_DIAGNOSTIC_TIMEOUT_MS,
    READ_FAILURE_DIAGNOSTIC,
    capture_file_read_diagnostic,
)
from browser_protocol import BrowserRuntimeError
from browser_trace import BrowserEngine
from browser import SOURCE, validate_index_policy


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
  cancel: () => mode === 'delayed-cancel-error' || mode === 'cleanup-race'
    ? new Promise((_resolve, reject) => setTimeout(
        () => reject(new DOMException('', 'AbortError')),
        mode === 'cleanup-race' ? 300 : 25
      ))
    : Promise.resolve(),
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


class FileDropTests(unittest.TestCase):
    def test_browser_target_resolution_covers_the_matrix(self):
        self.assertEqual(resolve_browser_target(None, "chrome"), (BrowserEngine.CHROMIUM, "chrome"))
        self.assertEqual(resolve_browser_target("chromium", "MicrosoftEdge"), (BrowserEngine.CHROMIUM, "MicrosoftEdge"))
        self.assertEqual(resolve_browser_target("firefox", None), (BrowserEngine.FIREFOX, "firefox"))
        self.assertEqual(resolve_browser_target("webkit", "safari"), (BrowserEngine.WEBKIT, "safari"))
        with self.assertRaisesRegex(BrowserRuntimeError, "incompatible"):
            resolve_browser_target("firefox", "chrome")

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

    def test_gallery_uses_real_host_and_external_consumer(self):
        validate_index_policy(SOURCE / "gallery.html")
        html = (SOURCE / "gallery.html").read_text(encoding="utf-8")
        script = (SOURCE / "gallery.js").read_text(encoding="utf-8")
        style = (SOURCE / "gallery.css").read_text(encoding="utf-8")
        self.assertIn('id="metis-app"', html)
        self.assertIn('src="./gallery.js"', html)
        self.assertEqual(html.count("<canvas "), 3)
        self.assertIn('import("./consumer/ritk_snap.js")', script)
        self.assertIn("start_web_orthogonal_canvases(", script)
        self.assertIn("stop_web_canvas()", script)
        self.assertNotIn("DataTransfer", script)
        self.assertNotIn("dispatchEvent", script)
        self.assertNotIn("fetch(", script)
        self.assertIn("#metis-app > :not(.metis-drop)", style)

    def test_transfer_observer_covers_drop_and_standard_chooser(self):
        self.assertIn("input.addEventListener('change'", OBSERVE_TRANSFER)
        self.assertIn("event.dataTransfer", OBSERVE_TRANSFER)
        self.assertIn("window.metisInputFiles", OBSERVE_TRANSFER)
        self.assertIn("window.metisSelectedFile = list.item ? list.item(0) : list[0]", OBSERVE_TRANSFER)
        self.assertIn("const evidence = (async () =>", OBSERVE_TRANSFER)
        self.assertIn("error.name", OBSERVE_TRANSFER)
        self.assertNotIn("dispatchEvent", OBSERVE_TRANSFER)

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
                raise BrowserRuntimeError("C:\\private\\study\\DICOMDIR")

        trace = types.SimpleNamespace(actions=[])
        capture_file_read_diagnostic(Client(), trace)
        self.assertEqual(
            trace.actions,
            [{"file_read_diagnostic": {"status": "unavailable", "reason": "webdriver-error"}}],
        )

    @staticmethod
    def _execute_diagnostic_fault(mode):
        process = subprocess.run(
            ["node", "-e", NODE_READ_DIAGNOSTIC_HARNESS, READ_FAILURE_DIAGNOSTIC, mode],
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
