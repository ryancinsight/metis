"""Authored application modules instantiate WASM and call one Rust host."""
from __future__ import annotations

import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]


class RustBrowserBootstrapTests(unittest.TestCase):
    def test_authored_modules_only_initialize_rust_hosts(self):
        import re
        for filename, module, export in (
            ("examples/browser/bootstrap.js", "metis_web", "metis_browser_start"),
            ("examples/browser/http-health.js", "metis_web", "metis_http_start"),
            ("crates/metis-starter/frontend/main.js", "metis_starter", "metis_starter_start"),
        ):
            script = (ROOT / filename).read_text(encoding="utf-8")
            pattern = (r'import init from "\./' + module +
                       r'\.js";\s*\(await init\(\)\)\.' + export + r'\(\);\s*')
            self.assertIsNotNone(re.fullmatch(pattern, script), filename)
