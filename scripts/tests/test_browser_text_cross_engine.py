"""Value-semantic tests for the cross-engine text geometry comparison."""
from __future__ import annotations

import copy
import json
import pathlib
import sys
import tempfile
import unittest

SCRIPTS = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from browser_protocol import BrowserRuntimeError
from browser_text_cross_engine import compare_text_geometry, main
from test_browser_text_geometry import _measurement


def _trace(engine: str) -> dict:
    """Build a passed schema-1 trace with two stable geometry observations."""
    first = _measurement()
    second = copy.deepcopy(first)
    return {
        "schema": 1,
        "status": "passed",
        "engine": engine,
        "metrics": {
            "text_geometry": [first, second],
            "text_geometry_stability": {
                "available": True,
                "stable": True,
                "observations": 2,
                "grapheme_clusters": len(first["grapheme_boundaries"]) - 1,
                "line_count": first["line_count"],
                "visual_order": first["visual_order"],
                "direction": first["style"]["direction"],
                "writing_mode": first["style"]["writing_mode"],
                "font_family": first["style"]["font_family"],
            },
        },
    }


class BrowserTextCrossEngineTests(unittest.TestCase):
    def test_matching_semantics_across_three_engines(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = []
            for engine in ("chromium", "firefox", "webkit"):
                path = pathlib.Path(directory) / f"{engine}.json"
                path.write_text(json.dumps(_trace(engine)), encoding="utf-8")
                paths.append((engine, path))
            result = compare_text_geometry(paths)
        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["engines"], ["chromium", "firefox", "webkit"])
        self.assertEqual(result["semantic_contract"]["line_count"], 2)

    def test_rejects_semantic_visual_order_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            chromium = pathlib.Path(directory) / "chromium.json"
            firefox = pathlib.Path(directory) / "firefox.json"
            chromium.write_text(json.dumps(_trace("chromium")), encoding="utf-8")
            changed = _trace("firefox")
            order = list(reversed(changed["metrics"]["text_geometry"][0]["visual_order"]))
            changed["metrics"]["text_geometry"][0]["visual_order"] = order
            changed["metrics"]["text_geometry"][1]["visual_order"] = order
            changed["metrics"]["text_geometry_stability"]["visual_order"] = order
            firefox.write_text(json.dumps(changed), encoding="utf-8")
            with self.assertRaisesRegex(BrowserRuntimeError, "visual_order differs"):
                compare_text_geometry([("chromium", chromium), ("firefox", firefox)], ("chromium", "firefox"))

    def test_rejects_missing_required_engine_and_duplicate(self):
        with tempfile.TemporaryDirectory() as directory:
            chromium = pathlib.Path(directory) / "chromium.json"
            firefox = pathlib.Path(directory) / "firefox.json"
            chromium.write_text(json.dumps(_trace("chromium")), encoding="utf-8")
            firefox.write_text(json.dumps(_trace("firefox")), encoding="utf-8")
            with self.assertRaisesRegex(BrowserRuntimeError, "missing required engines"):
                compare_text_geometry([("chromium", chromium), ("firefox", firefox)], ("chromium", "firefox", "webkit"))
            with self.assertRaisesRegex(BrowserRuntimeError, "duplicate engine"):
                compare_text_geometry([("chromium", chromium), ("chromium", chromium)], ("chromium",))

    def test_cli_writes_machine_readable_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "comparison.json"
            sys.argv = [
                "browser_text_cross_engine.py",
                "--trace",
                "chromium=missing.json",
                "--trace",
                "firefox=missing.json",
                "--output",
                str(output),
            ]
            self.assertEqual(main(), 1)
            self.assertEqual(json.loads(output.read_text(encoding="utf-8"))["status"], "failed")


if __name__ == "__main__":
    unittest.main()
