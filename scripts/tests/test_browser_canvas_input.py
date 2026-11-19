"""Path admission for browser raster input."""
import pathlib
import os
import sys
import unittest
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from browser_canvas_input import run
from browser_protocol import ROOT, BrowserRuntimeError
from browser_trace import BrowserEngine


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
