"""Browser input admission, gallery startup and screenshot contracts."""
from __future__ import annotations

import base64
import contextlib
import io
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import browser_drop
from browser_canvas_capture import pixel_observation
from browser_canvas_input import INSTALL_CANVAS_TRACE, _screenshot_dimensions, run
from browser_file_read import (
    CLEANUP_SELECTION_DIAGNOSTIC,
    INSTALL_SELECTION_DIAGNOSTIC,
    OBSERVE_SELECTION_DIAGNOSTIC,
    READ_FAILURE_DIAGNOSTIC,
    RESET_SELECTION_DIAGNOSTIC,
)
from browser_protocol import BrowserRuntimeError, ROOT
from browser_runtime import WAIT_FOR_SCRIPT
from browser_trace import BrowserEngine, DEVICE_SCALE_SCRIPT


NATIVE_WAIT_HARNESS = r"""
const script = process.argv[1];
const args = JSON.parse(process.argv[2]);
const scenario = JSON.parse(process.argv[3]);
let state = scenario.initial;
let observer, timer;
const result = {callbacks:[], observed:0, disconnected:0, cleared:0, delays:[]};
globalThis.document = {
  querySelector: (selector) => selector === '#gallery-status' && state !== null
    ? {textContent:state.text, getAttribute:name => name === 'data-state' ? state.marker : null}
    : null
};
globalThis.MutationObserver = class {
  constructor(callback) { observer = callback; }
  observe(_root, options) {
    if (!options.attributes || !options.characterData || !options.childList || !options.subtree)
      throw new Error('incomplete observation');
    result.observed++;
  }
  disconnect() { result.disconnected++; }
};
globalThis.window = {
  setTimeout: (callback, delay) => { timer = callback; result.delays.push(delay); return 51; },
  clearTimeout: (id) => { if (id !== 51) throw new Error('wrong timer'); result.cleared++; }
};
new Function(script)(...args, value => result.callbacks.push(value));
for (const step of scenario.steps) {
  if (step === 'timeout') { if (timer) timer(); }
  else { state = step; if (observer) observer(); }
}
process.stdout.write(JSON.stringify(result));
"""

# Valid one-pixel PNG for the isolated screenshot artifact path, not browser proof.
PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+XG1sAAAAASUVORK5CYII="
)


def _node_wait(script, args, initial, steps=()):
    result = subprocess.run(
        ["node", "-e", NATIVE_WAIT_HARNESS, script, json.dumps(args),
         json.dumps({"initial": initial, "steps": steps})],
        # Node writes UTF-8 JSON independently of the Python host locale.
        capture_output=True, check=True, encoding="utf-8", timeout=30,
    )
    return json.loads(result.stdout)


def _state(text, marker=None):
    return {"text": text, "marker": marker}


@unittest.skipUnless(shutil.which("node"), "Node.js is required for DOM-wait faults")
class GalleryDomWaitTests(unittest.TestCase):
    def _wait(self, initial, steps=(), *, opt_in=True):
        args = ["#gallery-status", "Ready", True, 30_000]
        if opt_in:
            args.append(512)
        return _node_wait(WAIT_FOR_SCRIPT, args, initial, steps)

    def test_already_failed_beats_stale_ready_without_arming_timeout(self):
        status = "Ready (stale): WebGPU adapter unavailable"
        read = self._wait(_state(status, "failed"), [_state("Ready"), "timeout"])
        self.assertEqual(read, {
            "callbacks": [{"ok": False, "failed": True, "status": status}],
            "observed": 0, "disconnected": 0, "cleared": 0, "delays": [],
        })

    def test_terminal_mutation_settles_once_and_disarms_timer(self):
        read = self._wait(None, [
            _state("Starting"), _state("Viewer could not start: requestDevice rejected", "failed"),
            _state("Ready"), "timeout",
        ])
        self.assertEqual(read, {
            "callbacks": [{"ok": False, "failed": True,
                           "status": "Viewer could not start: requestDevice rejected"}],
            "observed": 1, "disconnected": 1, "cleared": 1, "delays": [30_000],
        })

    def test_immediate_and_delayed_ready_keep_success_contract(self):
        self.assertEqual(self._wait(_state("Ready with WebGPU"))["callbacks"], [{"ok": True}])
        read = self._wait(None, [_state("Ready"), _state("failed later", "failed"), "timeout"])
        self.assertEqual(read, {
            "callbacks": [{"ok": True}],
            "observed": 1, "disconnected": 1, "cleared": 1, "delays": [30_000],
        })

    def test_pending_startup_keeps_genuine_timeout_and_ignores_late_ready(self):
        read = self._wait(_state("Starting"), ["timeout", _state("Ready"), "timeout"])
        self.assertEqual(read["callbacks"], [{"ok": False}])
        self.assertEqual((read["observed"], read["disconnected"], read["delays"]), (1, 1, [30_000]))

    def test_generic_waiter_does_not_opt_in_to_the_failed_marker(self):
        read = self._wait(_state("Ready (generic)", "failed"), opt_in=False)
        self.assertEqual(read["callbacks"], [{"ok": True}])
        read = self._wait(_state("failed (generic)", "failed"), ["timeout"], opt_in=False)
        self.assertEqual(read["callbacks"], [{"ok": False}])

    def test_dom_failure_witness_caps_the_wire_text(self):
        read = self._wait(_state("α" * 1200, "failed"))
        self.assertEqual(read["callbacks"], [{"ok": False, "failed": True, "status": "α" * 512}])


@unittest.skipUnless(shutil.which("node"), "Node.js is required for the real readiness script")
class GalleryFileSelectionPhaseTests(unittest.TestCase):
    def _run_one(self, initial, engine, browser_name):
        root = browser_drop.ROOT / "output"
        root.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="gallery-startup-", dir=root) as directory:
            path = pathlib.Path(directory)
            files = path / "fixture"
            files.mkdir()
            first = files / "slice01.bin"
            second = files / "slice02.bin"
            first.write_bytes(b"first test fixture")
            second.write_bytes(b"second test fixture")
            oracle = path / "oracle.json"
            oracle.write_text('{"viewer-canvas":{}}', encoding="utf-8")
            output = path / "run"
            args = browser_drop.build_parser().parse_args([
                "--driver-url", "http://127.0.0.1:9515", "--engine", engine,
                "--browser-name", browser_name, "--input", "chooser",
                "--files", str(files), "--oracle", str(oracle),
                "--consumer-revision", "1" * 40, "--output", str(output),
            ])
            calls = []
            driver = mock.Mock()
            driver.session_id = "isolated-unit-session"
            driver.capabilities = {"browserName": browser_name}
            driver.screenshot.return_value = PNG
            driver.find.side_effect = lambda selector: selector
            driver.send_file_paths.side_effect = lambda selector, paths: calls.append(
                (selector, [p.name for p in paths])
            )
            probes = []

            def execute_async(script, arguments=()):
                if script == WAIT_FOR_SCRIPT:
                    if arguments[0] == "#gallery-status":
                        result = _node_wait(script, arguments, initial, ["timeout"])
                        return result["callbacks"][0]
                    return {"ok": True}
                self.assertEqual(script, READ_FAILURE_DIAGNOSTIC)
                probes.append("read")
                return {"status": "captured", "whole_file": {"error_name": "NotReadableError"}}

            def execute(script, arguments=()):
                if script == DEVICE_SCALE_SCRIPT:
                    return {"device_pixel_ratio": 1, "inner_width": 1440, "inner_height": 1100}
                if script == browser_drop.OBSERVE_TRANSFER:
                    return {"visible": True}
                if "getElementById('drop-status')" in script:
                    return {"status": "selected", "bytes": "Byte access: failed"}
                if script == INSTALL_SELECTION_DIAGNOSTIC:
                    return {"id": "diagnostic-input"}
                if script == OBSERVE_SELECTION_DIAGNOSTIC:
                    return {"files": len(calls[-1][1]), "changes": 1}
                if script == RESET_SELECTION_DIAGNOSTIC:
                    return {"files": 0, "changes": 1}
                if script == CLEANUP_SELECTION_DIAGNOSTIC:
                    return {"removed": True, "restored": True}
                if script == browser_drop.CLEANUP_TRANSFER:
                    return {"listeners_removed": 4, "globals_cleared": True}
                self.assertIn("document.body.innerText.slice(0,8192)", script)
                return {"text": initial["text"][:100], "events": []}

            driver.execute_async.side_effect = execute_async
            driver.execute.side_effect = execute
            with mock.patch.object(browser_drop, "WebDriverClient", return_value=driver), \
                 mock.patch.object(browser_drop, "StaticServer",
                                   return_value=contextlib.nullcontext("http://127.0.0.1:1/")), \
                 contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaises(BrowserRuntimeError) as failure:
                    browser_drop.run(args)
            trace = json.loads((output / "trace.json").read_text(encoding="utf-8"))
            self.assertEqual(trace["error"], str(failure.exception))
            self.assertEqual(trace["status"], "failed")
            self.assertTrue(trace["cleanup"]["session_closed"])
            driver.close.assert_called_once_with()
            self.assertEqual((output / "failure.png").read_bytes(), PNG)
            self.assertEqual(trace["diagnostic"]["text"], initial["text"][:100])
            return trace, calls, probes

    def test_pre_ready_failure_never_sends_files_and_keeps_passive_artifacts(self):
        status = "Viewer could not start:\n\n no WebGPU adapter\t" + "α" * 600
        trace, calls, probes = self._run_one(_state(status, "failed"), "chromium", "chrome")
        self.assertEqual(calls, [])
        self.assertEqual(probes, ["read"])
        self.assertTrue(all("file_selection_diagnostic" not in a for a in trace["actions"]))
        self.assertTrue(trace["error"].startswith(
            "#gallery-status reported startup failure: Viewer could not start: no WebGPU adapter "
        ))
        self.assertNotIn("\n", trace["error"])
        self.assertNotIn("\t", trace["error"])
        error_status = trace["error"].partition(": ")[2]
        self.assertLessEqual(len(error_status.encode("utf-8")), 512)

    def test_pre_ready_timeout_retains_budget_and_never_sends_files(self):
        trace, calls, probes = self._run_one(_state("Starting"), "chromium", "chrome")
        self.assertIn("within 30000 ms", trace["error"])
        self.assertNotIn("reported startup failure", trace["error"])
        self.assertEqual(calls, [])
        self.assertEqual(probes, ["read"])

    def test_post_ready_safari_file_failure_keeps_passive_and_active_probes(self):
        trace, calls, probes = self._run_one(_state("Ready"), "webkit", "safari")
        self.assertIn("file transfer did not complete", trace["error"])
        self.assertEqual(calls, [
            ("#file-input", ["slice01.bin", "slice02.bin"]),
            ("#diagnostic-input", ["slice01.bin"]),
            ("#diagnostic-input", ["slice01.bin", "slice02.bin"]),
        ])
        self.assertEqual(probes, ["read", "read", "read"])
        active = next(a["file_selection_diagnostic"] for a in trace["actions"]
                      if "file_selection_diagnostic" in a)
        self.assertEqual(
            [(item["requested_files"], item["observation"]["files"])
             for item in active["selections"]], [(1, 1), (2, 2)]
        )
        self.assertEqual(active["cleanup"], {"removed": True, "restored": True})

class CanvasInputPathTests(unittest.TestCase):
    def test_linked_assets_are_rejected_before_browser_requests(self):
        with tempfile.TemporaryDirectory() as temporary:
            link = pathlib.Path(temporary) / "browser-assets"
            try:
                link.symlink_to(ROOT / "output" / "browser", target_is_directory=True)
            except OSError as error:
                if os.name == "nt" and error.winerror == 1314:
                    self.skipTest(f"host cannot create directory symlinks: {error}")
                raise
            with self.assertRaisesRegex(BrowserRuntimeError, "path is linked"):
                run("http://127.0.0.1:0", BrowserEngine.CHROMIUM, link, 1.0, 1000, "1" * 40)


@unittest.skipUnless(shutil.which("node"), "Node.js is required for listener instrumentation")
class CanvasInputTraceTests(unittest.TestCase):
    def test_trace_is_scoped_to_one_canvas_and_restores_its_methods(self):
        harness = r"""
const install = process.argv[1];
const canvas = new EventTarget();
const other = new EventTarget();
canvas.hasPointerCapture = () => true;
globalThis.window = {};
globalThis.document = {getElementById: id => id === 'canvas-input' ? canvas : null};
new Function(install)('canvas-input');
let canvasCalls = 0, otherCalls = 0;
const onCanvas = event => { canvasCalls++; event.preventDefault(); };
const onOther = () => { otherCalls++; };
canvas.addEventListener('pointerdown', onCanvas);
other.addEventListener('pointerdown', onOther);
const event = new Event('pointerdown', {cancelable: true});
Object.defineProperties(event, {
  pointerId: {value: 7}, button: {value: 0}, buttons: {value: 1}
});
canvas.dispatchEvent(event);
other.dispatchEvent(new Event('pointerdown'));
canvas.removeEventListener('pointerdown', onCanvas);
const state = window.canvasInputTrace.state();
window.canvasInputTrace.restore();
process.stdout.write(JSON.stringify({canvasCalls, otherCalls, state,
  ownAdd: Object.hasOwn(canvas, 'addEventListener'), traceRemoved: !window.canvasInputTrace}));
"""
        result = subprocess.run(
            ["node", "-e", harness, INSTALL_CANVAS_TRACE], capture_output=True,
            check=True, encoding="utf-8", timeout=30,
        )
        self.assertEqual(json.loads(result.stdout), {
            "canvasCalls": 1,
            "otherCalls": 1,
            "state": {
                "registrations": 1,
                "active": 0,
                "removals": 1,
                "observations": [{
                    "type": "pointerdown", "pointer_id": 7, "button": 0, "buttons": 1,
                    "trusted": False, "returned": True, "default_prevented": True,
                    "has_capture": True,
                }],
            },
            "ownAdd": False,
            "traceRemoved": True,
        })


class CanvasInputScreenshotTests(unittest.TestCase):
    def test_physical_extent_follows_display_scale(self):
        for scale, expected in ((1000, (100, 80)), (2000, (200, 160)),
                                (1250, (125, 100))):
            with self.subTest(scale=scale):
                self.assertEqual(_screenshot_dimensions(scale), expected)

    def test_fractional_edge_is_rejected_without_resampling(self):
        with self.assertRaisesRegex(BrowserRuntimeError, "fractional physical pixel"):
            _screenshot_dimensions(1333)

    def test_last_physical_pixel_at_double_scale_is_checked(self):
        frame = {"ok": True, "width": 200, "height": 160,
                 "pixels": [17, 34, 51, 255] * 31_999 + [17, 34, 50, 255]}
        with self.assertRaisesRegex(BrowserRuntimeError, r"pixel \(199, 159\)"):
            pixel_observation(frame, (200, 160), bytes((17, 34, 51, 255)) * 32_000)
