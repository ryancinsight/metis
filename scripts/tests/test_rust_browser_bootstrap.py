"""Authored application modules contain only the required WASM loader glue."""
from __future__ import annotations

import json
import pathlib
import shutil
import subprocess
import tomllib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]


class RustBrowserBootstrapTests(unittest.TestCase):
    def test_canvas_artifact_uses_only_the_canvas_module(self):
        manifest = tomllib.loads(
            (ROOT / "crates/metis-web/Cargo.toml").read_text(encoding="utf-8")
        )
        self.assertEqual(manifest["lib"]["crate-type"], ["cdylib", "rlib"])
        example = (ROOT / "crates/metis-web/examples/canvas_input.rs").read_text(
            encoding="utf-8"
        )
        browser = (
            ROOT / "crates/metis-web/examples/canvas_input/browser.rs"
        ).read_text(encoding="utf-8")
        self.assertIn('#[path = "../src/canvas/mod.rs"]', example)
        self.assertIn("use crate::canvas::", browser)
        self.assertNotIn("use metis_web::", browser)

    def test_authored_modules_only_initialize_rust_hosts(self):
        import re
        script = (ROOT / "examples/browser/bootstrap.js").read_text(encoding="utf-8")
        startup = (ROOT / "crates/metis-web/src/startup.rs").read_text(encoding="utf-8")
        self.assertIsNotNone(
            re.fullmatch(r'import init from "\./metis_web\.js";\s*await init\(\);\s*', script)
        )
        self.assertIn("#[wasm_bindgen::prelude::wasm_bindgen(start, private)]", startup)
        self.assertIn("fn initialize()", startup)
        self.assertNotIn("fn start()", startup)
        self.assertFalse((ROOT / "examples/browser/http-health.js").exists())
        for page, marker in (("index.html", "workbench"), ("http-health.html", "http")):
            document = (ROOT / "examples/browser" / page).read_text(encoding="utf-8")
            self.assertIn(f'data-metis-entry="{marker}"', document)
            self.assertIn('<script type="module" src="./bootstrap.js"></script>', document)

    def test_canvas_module_contains_only_initialization_and_bounded_failure_display(self):
        script = (ROOT / "examples/browser/canvas-input.js").read_text(encoding="utf-8")
        rust = (ROOT / "crates/metis-web/examples/canvas_input/browser.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("await init();", script)
        self.assertNotIn("canvas_mount", script)
        self.assertNotIn("addEventListener", script)
        self.assertNotIn("window.canvasInput", script)
        self.assertNotIn("EventTarget", script)
        self.assertIn("#[wasm_bindgen::prelude::wasm_bindgen(start)]", rust)
        self.assertIn("pub fn start()", rust)
        self.assertNotIn("canvas_mount", rust)
        self.assertNotIn("no_mangle", rust)

        manifest = tomllib.loads(
            (ROOT / "crates/metis-web/Cargo.toml").read_text(encoding="utf-8")
        )
        wasm = manifest["target"]['cfg(target_arch = "wasm32")']
        self.assertIn("wasm-bindgen", wasm["dependencies"])
        self.assertNotIn("dev-dependencies", wasm)

    @unittest.skipUnless(shutil.which("node"), "Node.js is required for module-loader faults")
    def test_canvas_module_initializes_once_and_caps_loader_failures(self):
        source = (ROOT / "examples/browser/canvas-input.js").read_text(encoding="utf-8")
        import_line = "import init from './canvas_input.js';"

        def run(initializer):
            script = source.replace(import_line, initializer, 1)
            harness = f"""
let starts = 0;
const status = {{textContent: 'loading'}};
globalThis.document = {{getElementById: id => id === 'canvas-input-status' ? status : null}};
{script}
process.stdout.write(JSON.stringify({{starts, status: status.textContent}}));
"""
            result = subprocess.run(
                ["node", "--input-type=module", "-e", harness], capture_output=True,
                check=True, encoding="utf-8", timeout=30,
            )
            return json.loads(result.stdout)

        initialized = run("const init = async () => { starts++; };")
        self.assertEqual(initialized, {"starts": 1, "status": "loading"})
        failed = run("const init = async () => { throw new Error('x'.repeat(700)); };")
        self.assertEqual(failed["starts"], 0)
        self.assertTrue(failed["status"].startswith("load-error:Error: "))
        self.assertEqual(len(failed["status"].removeprefix("load-error:")), 512)
